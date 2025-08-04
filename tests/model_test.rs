mod common;

use poe_api_process::get_model_list;

#[tokio::test]
async fn test_get_model_list() {
    common::setup();
    let result = get_model_list(Some("zh-Hant")).await;

    match result {
        Ok(models) => {
            assert!(!models.data.is_empty(), "Model list should not be empty");
            // Verify basic information of the first model
            if let Some(first_model) = models.data.first() {
                assert!(!first_model.id.is_empty(), "Model ID should not be empty");
                assert_eq!(first_model.object, "model", "Model type should be 'model'");
                assert_eq!(first_model.owned_by, "poe", "Model owner should be 'poe'");
            }
        }
        Err(e) => {
            panic!("Failed to get model list: {e}");
        }
    }
}