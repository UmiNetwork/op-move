use {
    crate::{json_utils::parse_params_0, jsonrpc::JsonRpcError},
    umi_app::{ApplicationReader, Dependencies},
};

pub enum ChainIdFormat {
    Hex,
    Decimal,
}

pub async fn execute<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
    format: ChainIdFormat,
) -> Result<serde_json::Value, JsonRpcError> {
    parse_params_0(request)?;
    let chain_id = app.chain_id();

    let response = match format {
        ChainIdFormat::Hex => format!("{chain_id:#x}"),
        ChainIdFormat::Decimal => chain_id.to_string(),
    };

    Ok(serde_json::Value::String(response))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::methods::tests::create_app};

    #[tokio::test]
    async fn test_execute() {
        let (reader, _app) = create_app();

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_chainId",
            "params": [],
            "id": 1
        });

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x194""#).unwrap();
        let actual_response = execute(request.clone(), &reader, ChainIdFormat::Hex)
            .await
            .unwrap();
        assert_eq!(actual_response, expected_response);

        let expected_response: serde_json::Value = serde_json::from_str(r#""404""#).unwrap();
        let actual_response = execute(request, &reader, ChainIdFormat::Decimal)
            .await
            .unwrap();
        assert_eq!(actual_response, expected_response);
    }
}
