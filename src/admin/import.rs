//! 凭据导入模块
//!
//! 支持从 kiro-accounts 导出格式导入凭据

use serde::Deserialize;

use crate::kiro::model::credentials::KiroCredentials;

/// kiro-accounts 导出文件格式
#[derive(Debug, Deserialize)]
pub struct KiroAccountsExport {
    #[serde(rename = "version")]
    pub version: Option<String>,

    #[serde(rename = "exportedAt")]
    pub exported_at: Option<i64>,

    pub accounts: Vec<KiroAccount>,
}

/// kiro-accounts 单个账号
#[derive(Debug, Deserialize)]
pub struct KiroAccount {
    pub email: Option<String>,
    pub credentials: KiroAccountCredentials,

    #[serde(rename = "machineId", default)]
    pub machine_id: Option<String>,

    #[serde(default)]
    pub subscription: Option<KiroSubscription>,

    #[serde(default)]
    pub tags: Vec<String>,
}

/// kiro-accounts 凭据信息
#[derive(Debug, Deserialize)]
pub struct KiroAccountCredentials {
    #[serde(rename = "accessToken", default)]
    pub access_token: Option<String>,

    #[serde(rename = "refreshToken", default)]
    pub refresh_token: Option<String>,

    #[serde(rename = "clientId", default)]
    pub client_id: Option<String>,

    #[serde(rename = "clientSecret", default)]
    pub client_secret: Option<String>,

    #[serde(default)]
    pub region: Option<String>,

    #[serde(rename = "expiresAt", default)]
    pub expires_at: Option<i64>,

    #[serde(rename = "authMethod", default)]
    pub auth_method: Option<String>,
}

/// kiro-accounts 订阅信息
#[derive(Debug, Deserialize, Default)]
pub struct KiroSubscription {
    #[serde(rename = "type", default)]
    pub sub_type: Option<String>,

    #[serde(default)]
    pub title: Option<String>,
}

/// 导入结果
#[derive(Debug, serde::Serialize)]
pub struct ImportResult {
    pub success: bool,
    pub message: String,
    pub imported: u32,
    pub updated: u32,
    pub skipped: u32,
    pub total: u32,
}

impl ImportResult {
    pub fn new(imported: u32, updated: u32, skipped: u32, total: u32) -> Self {
        let message = format!(
            "导入完成: 新增 {}, 更新 {}, 跳过 {}, 共处理 {}",
            imported, updated, skipped, total
        );
        Self {
            success: true,
            message,
            imported,
            updated,
            skipped,
            total,
        }
    }
}

// ============ 格式转换函数 ============

/// 将毫秒时间戳转换为 RFC3339 字符串
fn timestamp_to_rfc3339(timestamp_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(timestamp_ms)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339())
}

/// 规范化认证方式
fn normalize_auth_method(method: Option<&str>) -> Option<String> {
    method.and_then(|m| {
        if m.is_empty() {
            return None;
        }
        Some(match m.to_lowercase().as_str() {
            "idc" | "builderid" | "iam" => "idc".to_string(),
            "social" => "social".to_string(),
            other => other.to_string(),
        })
    })
}

/// 过滤空字符串，转为 None
fn filter_empty(s: Option<String>) -> Option<String> {
    s.filter(|v| !v.is_empty())
}

/// 解析导入数据，支持多种格式
pub fn parse_import_data(json_str: &str) -> Result<Vec<KiroAccount>, String> {
    // 尝试解析为完整导出格式
    if let Ok(export) = serde_json::from_str::<KiroAccountsExport>(json_str) {
        return Ok(export.accounts);
    }

    // 尝试解析为数组格式
    if let Ok(accounts) = serde_json::from_str::<Vec<KiroAccount>>(json_str) {
        return Ok(accounts);
    }

    Err("无法解析导入数据：期望 KiroAccountsExport 格式或 KiroAccount 数组".to_string())
}

impl KiroAccount {
    /// 转换为内部凭据格式
    pub fn to_credentials(&self) -> KiroCredentials {
        KiroCredentials {
            id: None,
            access_token: filter_empty(self.credentials.access_token.clone()),
            refresh_token: filter_empty(self.credentials.refresh_token.clone()),
            profile_arn: None,
            expires_at: self.credentials.expires_at.map(timestamp_to_rfc3339),
            auth_method: normalize_auth_method(self.credentials.auth_method.as_deref()),
            client_id: filter_empty(self.credentials.client_id.clone()),
            client_secret: filter_empty(self.credentials.client_secret.clone()),
            priority: 0,
            region: filter_empty(self.credentials.region.clone()),
            auth_region: None,
            api_region: None,
            machine_id: filter_empty(self.machine_id.clone()),
            email: filter_empty(self.email.clone()),
            subscription_title: self.subscription.as_ref().and_then(|s| filter_empty(s.title.clone())),
            proxy_url: None,
            proxy_username: None,
            proxy_password: None,
            disabled: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_kiro_accounts_export() {
        let json = r#"{
            "version": "1.5.0",
            "exportedAt": 1771429590941,
            "accounts": [
                {
                    "email": "test@example.com",
                    "credentials": {
                        "clientId": "test-client-id",
                        "clientSecret": "test-secret",
                        "region": "us-east-1",
                        "expiresAt": 1771433116224,
                        "authMethod": "IdC"
                    }
                }
            ]
        }"#;

        let accounts = parse_import_data(json).unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].email, Some("test@example.com".to_string()));
        assert_eq!(
            accounts[0].credentials.client_id,
            Some("test-client-id".to_string())
        );
    }

    #[test]
    fn test_parse_accounts_array() {
        let json = r#"[
            {
                "email": "user1@example.com",
                "credentials": {
                    "clientId": "client1",
                    "region": "us-east-1"
                }
            },
            {
                "email": "user2@example.com",
                "credentials": {
                    "clientId": "client2",
                    "region": "eu-west-1"
                }
            }
        ]"#;

        let accounts = parse_import_data(json).unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].email, Some("user1@example.com".to_string()));
        assert_eq!(
            accounts[1].credentials.region,
            Some("eu-west-1".to_string())
        );
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = r#"{"invalid": true}"#;
        let result = parse_import_data(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_timestamp_to_rfc3339() {
        let result = timestamp_to_rfc3339(1771433116224);
        assert!(result.contains("2026"));
    }

    #[test]
    fn test_normalize_auth_method_idc() {
        assert_eq!(
            normalize_auth_method(Some("IdC")),
            Some("idc".to_string())
        );
        assert_eq!(
            normalize_auth_method(Some("BuilderId")),
            Some("idc".to_string())
        );
        assert_eq!(normalize_auth_method(Some("IAM")), Some("idc".to_string()));
        assert_eq!(
            normalize_auth_method(Some("builderid")),
            Some("idc".to_string())
        );
    }

    #[test]
    fn test_normalize_auth_method_social() {
        assert_eq!(
            normalize_auth_method(Some("Social")),
            Some("social".to_string())
        );
        assert_eq!(
            normalize_auth_method(Some("social")),
            Some("social".to_string())
        );
    }

    #[test]
    fn test_normalize_auth_method_none() {
        assert_eq!(normalize_auth_method(None), None);
        assert_eq!(normalize_auth_method(Some("")), None);
    }

    #[test]
    fn test_filter_empty() {
        assert_eq!(filter_empty(Some("test".to_string())), Some("test".to_string()));
        assert_eq!(filter_empty(Some("".to_string())), None);
        assert_eq!(filter_empty(None), None);
    }

    #[test]
    fn test_to_credentials() {
        let account = KiroAccount {
            email: Some("test@example.com".to_string()),
            credentials: KiroAccountCredentials {
                access_token: None,
                refresh_token: Some("refresh-token-123".to_string()),
                client_id: Some("client-abc".to_string()),
                client_secret: Some("secret-xyz".to_string()),
                region: Some("us-east-1".to_string()),
                expires_at: Some(1771433116224),
                auth_method: Some("IdC".to_string()),
            },
            machine_id: Some("machine123".to_string()),
            subscription: Some(KiroSubscription {
                sub_type: Some("Free".to_string()),
                title: Some("KIRO FREE".to_string()),
            }),
            tags: vec![],
        };

        let creds = account.to_credentials();
        assert_eq!(creds.email, Some("test@example.com".to_string()));
        assert_eq!(creds.client_id, Some("client-abc".to_string()));
        assert_eq!(creds.auth_method, Some("idc".to_string()));
        assert_eq!(creds.region, Some("us-east-1".to_string()));
        assert_eq!(creds.machine_id, Some("machine123".to_string()));
        assert_eq!(creds.subscription_title, Some("KIRO FREE".to_string()));
        assert!(creds.expires_at.unwrap().contains("2026"));
    }

    #[test]
    fn test_to_credentials_empty_strings() {
        let account = KiroAccount {
            email: Some("".to_string()), // empty
            credentials: KiroAccountCredentials {
                access_token: Some("".to_string()), // empty
                refresh_token: Some("".to_string()), // empty
                client_id: Some("client-abc".to_string()),
                client_secret: Some("secret-xyz".to_string()),
                region: Some("".to_string()), // empty
                expires_at: None,
                auth_method: Some("".to_string()), // empty
            },
            machine_id: None,
            subscription: None,
            tags: vec![],
        };

        let creds = account.to_credentials();
        assert_eq!(creds.email, None);
        assert_eq!(creds.access_token, None);
        assert_eq!(creds.refresh_token, None);
        assert_eq!(creds.region, None);
        assert_eq!(creds.auth_method, None);
        assert_eq!(creds.client_id, Some("client-abc".to_string()));
    }

    #[test]
    fn test_import_result() {
        let result = ImportResult::new(5, 3, 0, 8);
        assert_eq!(result.imported, 5);
        assert_eq!(result.updated, 3);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.total, 8);
        assert!(result.success);
        assert!(result.message.contains("新增 5"));
        assert!(result.message.contains("更新 3"));
    }
}
