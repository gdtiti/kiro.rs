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

    #[serde(rename = "machineId")]
    pub machine_id: Option<String>,

    pub subscription: Option<KiroSubscription>,

    #[serde(default)]
    pub tags: Vec<String>,
}

/// kiro-accounts 凭据信息
#[derive(Debug, Deserialize)]
pub struct KiroAccountCredentials {
    #[serde(rename = "accessToken")]
    pub access_token: Option<String>,

    #[serde(rename = "refreshToken")]
    pub refresh_token: Option<String>,

    #[serde(rename = "clientId")]
    pub client_id: Option<String>,

    #[serde(rename = "clientSecret")]
    pub client_secret: Option<String>,

    pub region: Option<String>,

    #[serde(rename = "expiresAt")]
    pub expires_at: Option<i64>,

    #[serde(rename = "authMethod")]
    pub auth_method: Option<String>,
}

/// kiro-accounts 订阅信息
#[derive(Debug, Deserialize)]
pub struct KiroSubscription {
    #[serde(rename = "type")]
    pub sub_type: Option<String>,

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
    method.map(|m| match m.to_lowercase().as_str() {
        "idc" | "builderid" | "iam" => "idc".to_string(),
        "social" => "social".to_string(),
        other => other.to_string(),
    })
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
            access_token: self.credentials.access_token.clone(),
            refresh_token: self.credentials.refresh_token.clone(),
            profile_arn: None,
            expires_at: self.credentials.expires_at.map(timestamp_to_rfc3339),
            auth_method: normalize_auth_method(self.credentials.auth_method.as_deref()),
            client_id: self.credentials.client_id.clone(),
            client_secret: self.credentials.client_secret.clone(),
            priority: 0,
            region: self.credentials.region.clone(),
            auth_region: None,
            api_region: None,
            machine_id: self.machine_id.clone(),
            email: self.email.clone(),
            subscription_title: self.subscription.as_ref().and_then(|s| s.title.clone()),
            proxy_url: None,
            proxy_username: None,
            proxy_password: None,
            disabled: false,
        }
    }
}
