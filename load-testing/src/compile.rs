use {
    crate::CARGO_MANIFEST_DIR,
    std::{
        fs::File,
        path::{Path, PathBuf},
    },
    tokio::process::Command,
};

pub async fn build_umi_server() -> anyhow::Result<PathBuf> {
    let workspace_root = Path::new(CARGO_MANIFEST_DIR)
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Failed to get workspace root from {CARGO_MANIFEST_DIR}"))?;

    let log_file = File::create(Path::new(CARGO_MANIFEST_DIR).join("op_move.log"))?;

    let compile_process = Command::new("cargo")
        .current_dir(workspace_root)
        .args([
            "build",
            "-p",
            "umi-server",
            "--release",
            "--features",
            "storage",
        ])
        .stdout(log_file)
        .spawn()?;

    let output = compile_process.wait_with_output().await?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("umi-server failed to compile: stdout={stdout} stderr={stderr}");
    }

    let binary = workspace_root
        .join("target")
        .join("release")
        .join("op-move");
    Ok(binary)
}
