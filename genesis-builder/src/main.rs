use {
    move_package::LintFlag,
    move_package::{package_hooks::register_package_hooks, BuildConfig as MoveBuildConfig},
    once_cell::sync::Lazy,
    std::env::temp_dir,
    std::fs,
    std::io::{BufRead, BufReader},
    std::path::{Path, PathBuf},
    std::process::Command,
    sui_framework::SystemPackage,
    sui_move_build::{BuildConfig, SuiPackageHooks},
    sui_types::base_types::ObjectID,
};

pub const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");
pub const APTOS_SNAPSHOT_NAME: &str = "aptos.mrb";
pub const SUI_SNAPSHOT_NAME: &str = "sui.mrb";
pub const MOVE_STDLIB_FOLDER_NAME: &str = "move-stdlib";
pub const SUI_FRAMEWORK_FOLDER_NAME: &str = "sui-framework";
pub const SUI_STDLIB_PACKAGE_ID: ObjectID = small_object_id(0x21);
pub const SUI_FRAMEWORK_PACKAGE_ID: ObjectID = small_object_id(0x22);

const ORIGINAL_SUI_STDLIB_PACKAGE_ID: ObjectID = small_object_id(0x1);
const ORIGINAL_SUI_FRAMEWORK_PACKAGE_ID: ObjectID = small_object_id(0x2);
const SUI_REPO: &str = "https://github.com/MystenLabs/sui";
const SUI_REPO_TAG: &str = "testnet-v1.28.3";
const MOVE_TOML: &str = "Move.toml";

static DIR: Lazy<PathBuf> = Lazy::new(|| {
    temp_dir()
        .join("sui")
        .join("crates")
        .join("sui-framework")
        .join("packages")
});
static COIN_PATH: Lazy<PathBuf> = Lazy::new(|| {
    DIR.join(SUI_FRAMEWORK_FOLDER_NAME)
        .join("sources")
        .join("coin.move")
});

const fn small_object_id(value: u8) -> ObjectID {
    ObjectID::from_single_byte(value)
}

fn main() -> anyhow::Result<()> {
    println!("Starting genesis package generation");
    register_package_hooks(Box::new(SuiPackageHooks));

    bring_in_sui_packages()?;
    build_sui_packages()?;
    Ok(())
}

fn bring_in_sui_packages() -> anyhow::Result<()> {
    // Clone the Sui repo which contains the framework packages
    Command::new("git")
        .current_dir(&temp_dir())
        .args(["clone", "--depth", "1", "--branch", SUI_REPO_TAG, SUI_REPO])
        .output()
        .unwrap();

    // Addresses are mapped from 0x1, 0x2 to 0x21, 0x22 for conflict resolution
    let move_toml_file = &DIR.join(MOVE_STDLIB_FOLDER_NAME).join(MOVE_TOML);
    let content = fs::read_to_string(move_toml_file)?;
    let content = content.replace(
        &ORIGINAL_SUI_STDLIB_PACKAGE_ID.to_hex_literal(),
        &SUI_STDLIB_PACKAGE_ID.to_hex_literal(),
    );
    fs::write(move_toml_file, content)?;
    let move_toml_file = &DIR.join(SUI_FRAMEWORK_FOLDER_NAME).join(MOVE_TOML);
    let content = fs::read_to_string(move_toml_file)?;
    let content = content.replace(
        &ORIGINAL_SUI_FRAMEWORK_PACKAGE_ID.to_hex_literal(),
        &SUI_FRAMEWORK_PACKAGE_ID.to_hex_literal(),
    );
    fs::write(move_toml_file, content)?;

    // Remove #[deprecated(..)] annotations as they cause Move compilation issues
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(COIN_PATH.as_path())
        .expect("coin.move file doesn't exist");
    let lines = BufReader::new(file)
        .lines()
        .map(|line| line.unwrap())
        .filter(|line| !line.contains("#[deprecated"))
        .collect::<Vec<String>>()
        .join("\n");
    fs::write(COIN_PATH.as_path(), lines).expect("Can't write to `coin` module");
    Ok(())
}

fn build_sui_packages() -> anyhow::Result<()> {
    let config = MoveBuildConfig {
        generate_docs: true,
        warnings_are_errors: true,
        install_dir: Some(PathBuf::from(".")),
        lint_flag: LintFlag::LEVEL_NONE,
        default_edition: None,
        ..Default::default()
    };
    debug_assert!(!config.test_mode);

    build_packages_with_move_config(config)?;
    Ok(())
}

fn build_packages_with_move_config(config: MoveBuildConfig) -> anyhow::Result<()> {
    let build_config = BuildConfig {
        config: config.clone(),
        run_bytecode_verifier: false,
        print_diags_to_stderr: false,
    };

    let move_stdlib_path = &DIR.join(MOVE_STDLIB_FOLDER_NAME);
    let sui_framework_path = &DIR.join(SUI_FRAMEWORK_FOLDER_NAME);
    let stdlib_cmp_pkg = build_config.clone().build(move_stdlib_path)?;
    let framework_cmp_pkg = build_config.build(sui_framework_path)?;

    let packages = vec![
        SystemPackage {
            id: SUI_STDLIB_PACKAGE_ID,
            bytes: stdlib_cmp_pkg.get_package_bytes(false),
            dependencies: vec![],
        },
        SystemPackage {
            id: SUI_FRAMEWORK_PACKAGE_ID,
            bytes: framework_cmp_pkg.get_package_bytes(false),
            dependencies: vec![SUI_STDLIB_PACKAGE_ID],
        },
    ];

    // Serialize packages and write to a single file
    let binary = bcs::to_bytes(&packages)?;
    fs::write(Path::new(CRATE_ROOT).join(SUI_SNAPSHOT_NAME), binary)?;
    println!("Generated Sui framework packages");
    Ok(())
}
