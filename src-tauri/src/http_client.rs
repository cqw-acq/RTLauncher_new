use reqwest::header;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OnceCell, Semaphore};
pub const CURSEFORGE_API_KEY: &str = "$2a$10$VTAFCxje5a1Jkqv0aGWjQ.fULedAEPctDqppOkNMRVv.edVnG7KQ6";
pub const DEFAULT_MAX_CONCURRENT: usize = 256;
static SHARED_CLIENT: OnceCell<Arc<reqwest::Client>> = OnceCell::const_new();
static MODRINTH_CLIENT: OnceCell<Arc<reqwest::Client>> = OnceCell::const_new();
static CURSEFORGE_CLIENT: OnceCell<Arc<reqwest::Client>> = OnceCell::const_new();
static GLOBAL_SEMAPHORE: OnceCell<Arc<Semaphore>> = OnceCell::const_new();
fn base_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (compatible; RTLauncher/1.0; +https://github.com/bubulaladdi/RTLauncher)",
        )
        .connect_timeout(Duration::from_secs(15))
        .pool_idle_timeout(Duration::from_secs(300))
        .pool_max_idle_per_host(128)
        .tcp_nodelay(true)
        .http1_title_case_headers()
        .tcp_keepalive(Some(Duration::from_secs(30)))
        .redirect(reqwest::redirect::Policy::limited(8))
}
pub async fn shared_client() -> Arc<reqwest::Client> {
    SHARED_CLIENT
        .get_or_init(|| async {
            Arc::new(
                base_client_builder()
                    .timeout(Duration::from_secs(600))
                    .build()
                    .expect("Failed to build shared HTTP client"),
            )
        })
        .await
        .clone()
}
pub async fn curseforge_client() -> Arc<reqwest::Client> {
    CURSEFORGE_CLIENT
        .get_or_init(|| async {
            let mut headers = header::HeaderMap::new();
            if let Ok(value) = header::HeaderValue::from_str(CURSEFORGE_API_KEY) {
                headers.insert(header::HeaderName::from_static("x-api-key"), value);
            }
            Arc::new(
                base_client_builder()
                    .default_headers(headers)
                    .timeout(Duration::from_secs(600))
                    .build()
                    .expect("Failed to build CurseForge HTTP client"),
            )
        })
        .await
        .clone()
}
pub async fn modrinth_client() -> Arc<reqwest::Client> {
    MODRINTH_CLIENT
        .get_or_init(|| async {
            let mut headers = header::HeaderMap::new();
            if let Ok(api_value) = "v2".parse::<header::HeaderValue>() {
                headers.insert(
                    header::HeaderName::from_static("x-modrinth-api-version"),
                    api_value,
                );
            }
            Arc::new(
                base_client_builder()
                    .default_headers(headers)
                    .timeout(Duration::from_secs(300))
                    .build()
                    .expect("Failed to build Modrinth HTTP client"),
            )
        })
        .await
        .clone()
}
pub async fn global_semaphore() -> Arc<Semaphore> {
    GLOBAL_SEMAPHORE
        .get_or_init(|| async { Arc::new(Semaphore::new(DEFAULT_MAX_CONCURRENT)) })
        .await
        .clone()
}
pub async fn resolve_redirect_url(url: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (compatible; RTLauncher/1.0; +https://github.com/bubulaladdi/RTLauncher)",
        )
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()
        .map_err(|e| format!("构建客户端失败: {}", e))?;
    let resp = client
        .head(url)
        .send()
        .await
        .map_err(|e| format!("HEAD 请求失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}: {}", resp.status(), url));
    }
    let final_url = resp.url().to_string();
    if final_url != url {
        return Ok(final_url);
    }
    Ok(url.to_string())
}
#[derive(Clone, Copy)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
}
impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 800,
            max_delay_ms: 5000,
        }
    }
}
pub async fn get_with_retry(
    client: &reqwest::Client,
    url: &str,
    config: Option<RetryConfig>,
) -> Result<reqwest::Response, String> {
    let cfg = config.unwrap_or_default();
    let mut last_error: Option<String> = None;
    for attempt in 0..=cfg.max_retries {
        match client.get(url).send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_server_error() {
                    last_error = Some(format!("HTTP {}", status));
                } else if status.is_client_error() && attempt == cfg.max_retries {
                    return Err(format!("HTTP {}", status));
                } else if status.is_client_error() {
                    return Err(format!("HTTP {}", status));
                } else {
                    return Ok(response);
                }
            }
            Err(e) => {
                last_error = Some(e.to_string());
            }
        }
        if attempt >= cfg.max_retries {
            break;
        }
        let delay_ms = std::cmp::min(cfg.initial_delay_ms * (1u64 << attempt), cfg.max_delay_ms);
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }
    Err(last_error.unwrap_or_else(|| "请求失败".to_string()))
}
pub mod curseforge_class_ids {
    pub const MOD: u32 = 6;
    pub const RESOURCE_PACK: u32 = 12;
    pub const WORLD: u32 = 17;
    pub const MODPACK_CANDIDATES: &[u32] = &[4471, 4473];
    pub const SHADER_CANDIDATES: &[u32] = &[6552];
    pub const DATAPACK_CANDIDATES: &[u32] = &[6945, 6949];
    pub const TYPE_MOD: &str = "mod";
    pub const TYPE_MODPACK: &str = "modpack";
    pub const TYPE_RESOURCE_PACK: &str = "resourcepack";
    pub const TYPE_SHADER: &str = "shader";
    pub const TYPE_DATAPACK: &str = "datapack";
    pub const TYPE_WORLD: &str = "world";
    pub fn candidates_for_type(type_name: &str) -> Vec<u32> {
        let t = type_name.to_lowercase();
        match t.as_str() {
            n if n == TYPE_MOD || n == "mods" => vec![MOD],
            n if n == TYPE_MODPACK || n == "modpacks" => MODPACK_CANDIDATES.to_vec(),
            n if n == TYPE_RESOURCE_PACK
                || n == "resource_pack"
                || n == "resource-pack"
                || n == "resourcepacks"
                || n == "resource_packs"
                || n == "resource-packs"
                || n == "texturepack"
                || n == "texture_pack"
                || n == "texture-pack"
                || n == "texturepacks"
                || n == "texture_packs"
                || n == "texture-packs" =>
            {
                vec![RESOURCE_PACK]
            }
            n if n == TYPE_SHADER
                || n == "shaders"
                || n == "shaderpack"
                || n == "shader_pack"
                || n == "shader-pack"
                || n == "shaderpacks"
                || n == "shader_packs"
                || n == "shader-packs" => SHADER_CANDIDATES.to_vec(),
            n if n == TYPE_DATAPACK
                || n == "data_pack"
                || n == "data-pack"
                || n == "datapacks"
                || n == "data_packs"
                || n == "data-packs" => DATAPACK_CANDIDATES.to_vec(),
            n if n == TYPE_WORLD || n == "worlds" || n == "map" || n == "maps" => vec![WORLD],
            _ => all(),
        }
    }
    pub fn matches_type(class_id: Option<u32>, website_url: &str, type_name: &str) -> bool {
        let t = type_name.to_lowercase();
        if let Some(cid) = class_id {
            let candidates = candidates_for_type(&t);
            if candidates.contains(&cid) {
                return true;
            }
        }
        let url = website_url.to_lowercase();
        if t == TYPE_MODPACK
            || t == "modpacks"
        {
            url.contains("/modpacks/")
        } else if t == TYPE_RESOURCE_PACK
            || t == "resource_pack"
            || t == "resource-pack"
            || t == "resourcepacks"
            || t == "resource_packs"
            || t == "resource-packs"
            || t == "texturepack"
            || t == "texture_pack"
            || t == "texture-pack"
            || t == "texturepacks"
            || t == "texture_packs"
            || t == "texture-packs"
        {
            url.contains("/texture-packs/") || url.contains("/resource-packs/")
        } else if t == TYPE_SHADER
            || t == "shaders"
            || t == "shaderpack"
            || t == "shader_pack"
            || t == "shader-pack"
            || t == "shaderpacks"
            || t == "shader_packs"
            || t == "shader-packs"
        {
            url.contains("/shaders/")
        } else if t == TYPE_DATAPACK
            || t == "data_pack"
            || t == "data-pack"
            || t == "datapacks"
            || t == "data_packs"
            || t == "data-packs"
        {
            url.contains("/data-packs/")
        } else if t == TYPE_WORLD || t == "worlds" || t == "map" || t == "maps" {
            url.contains("/worlds/")
        } else if t == TYPE_MOD || t == "mods" {
            url.contains("/mc-mods/") || url.contains("/mods/")
        } else {
            false
        }
    }
    pub fn all() -> Vec<u32> {
        let mut v = vec![MOD, RESOURCE_PACK, WORLD];
        v.extend(MODPACK_CANDIDATES);
        v.extend(SHADER_CANDIDATES);
        v.extend(DATAPACK_CANDIDATES);
        v
    }
}