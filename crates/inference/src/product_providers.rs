use crate::fashion::{FoundProduct, Outcome};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use frameiq_contracts::*;
use serde_json::{json, Value};
use std::time::Duration;
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub provider: String,
    pub state: String,
    pub reviewed_images: bool,
    pub hosted_search: bool,
    pub paid_fallback: bool,
}
pub trait ProductDiscoveryProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider: self.id().into(),
            state: "account_policy_dependent".into(),
            reviewed_images: true,
            hosted_search: true,
            paid_fallback: false,
        }
    }
    fn evidence(&self, outcome: &Outcome) -> Vec<(String, String)> {
        outcome.sources.clone()
    }

    fn id(&self) -> &str;
    fn search(
        &self,
        model: &str,
        images: &[Vec<u8>],
        context: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> AppResult<Outcome>;
}
pub struct ChatGptSearch<'a> {
    pub token: &'a str,
}
impl ProductDiscoveryProvider for ChatGptSearch<'_> {
    fn id(&self) -> &str {
        "chatgpt"
    }
    fn search(
        &self,
        model: &str,
        images: &[Vec<u8>],
        context: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> AppResult<Outcome> {
        crate::fashion::request(
            self.token,
            model,
            &FashionTaskV1::Research,
            images,
            context,
            cancelled,
        )
    }
}
pub struct EbaySearch<'a> {
    pub credentials: &'a str,
}
fn invalid() -> AppError {
    AppError::new(
        "SHOPPING_PROVIDER",
        "The shopping provider returned an unsupported response.",
    )
}
async fn bounded(mut r: reqwest::Response) -> AppResult<Value> {
    if !r.status().is_success() {
        return Err(match r.status().as_u16(){401=>AppError::new("SHOPPING_AUTH","The eBay credentials were rejected. Update them in Settings."),403=>AppError::new("SHOPPING_ACCESS","The eBay application does not have this production capability. Use ChatGPT or local catalog search."),429=>AppError::new("SHOPPING_QUOTA","The eBay request limit was reached. No automatic retry was made."),_=>AppError::new("SHOPPING_PROVIDER","The eBay request failed. No automatic retry was made.")});
    }
    let mut bytes = Vec::new();
    while let Some(c) = r.chunk().await.map_err(|_| invalid())? {
        if bytes.len() + c.len() > 2 * 1024 * 1024 {
            return Err(invalid());
        }
        bytes.extend_from_slice(&c);
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}
impl ProductDiscoveryProvider for EbaySearch<'_> {
    fn id(&self) -> &str {
        "ebay"
    }
    fn search(
        &self,
        _model: &str,
        images: &[Vec<u8>],
        _context: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> AppResult<Outcome> {
        let credentials: Value = serde_json::from_str(self.credentials).map_err(|_| invalid())?;
        let id = credentials["clientId"].as_str().ok_or_else(invalid)?;
        let secret = credentials["clientSecret"].as_str().ok_or_else(invalid)?;
        if id.is_empty()
            || secret.is_empty()
            || id.len() > 4096
            || secret.len() > 4096
            || images.len() != 1
            || images[0].len() > 1024 * 1024
        {
            return Err(invalid());
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| invalid())?;
        rt.block_on(async{let work=async{if cancelled(){return Err(AppError::new("CANCELLED","Search cancelled before dispatch."));}let c=reqwest::Client::builder().https_only(true).no_proxy().redirect(reqwest::redirect::Policy::none()).retry(reqwest::retry::never()).connect_timeout(Duration::from_secs(5)).timeout(Duration::from_secs(90)).build().map_err(|_|invalid())?;
   let auth=bounded(c.post("https://api.ebay.com/identity/v1/oauth2/token").basic_auth(id,Some(secret)).form(&[("grant_type","client_credentials"),("scope","https://api.ebay.com/oauth/api_scope")]).send().await.map_err(|_|invalid())?).await?;let token=auth["access_token"].as_str().ok_or_else(invalid)?;
   let value=bounded(c.post("https://api.ebay.com/buy/browse/v1/item_summary/search_by_image").bearer_auth(token).header("X-EBAY-C-MARKETPLACE-ID","EBAY_US").query(&[("limit","20")]).json(&json!({"image":STANDARD.encode(&images[0])})).send().await.map_err(|_|invalid())?).await?;
   parse_ebay(&value)
  };tokio::select!{r=work=>r,_=async{while !cancelled(){tokio::time::sleep(Duration::from_millis(25)).await;}}=>Err(AppError::new("CANCELLED","Search cancelled. The reviewed crop may already have been sent."))}})
    }
}
pub fn parse_ebay(v: &Value) -> AppResult<Outcome> {
    let empty = Vec::new();
    let rows = v["itemSummaries"].as_array().unwrap_or(&empty);
    let mut products = Vec::new();
    let mut sources = Vec::new();
    for r in rows.iter().take(20) {
        let Some(url) = r["itemWebUrl"]
            .as_str()
            .filter(|u| crate::fashion::valid_url(u))
        else {
            continue;
        };
        let Some(name) = r["title"].as_str() else {
            continue;
        };
        if name.len() > 500 {
            continue;
        }
        let price = r["price"]["value"]
            .as_str()
            .filter(|p| p.parse::<f64>().is_ok_and(|n| n.is_finite() && n >= 0.))
            .map(str::to_string);
        let currency = r["price"]["currency"].as_str().map(str::to_string);
        sources.push((url.to_string(), name.to_string()));
        products.push(FoundProduct{name:name.into(),brand:None,style_id:None,category:"unknown".into(),url:url.into(),image_url:r["image"]["imageUrl"].as_str().map(str::to_string),retailer:"eBay".into(),price,currency,availability:None,source_excerpt:serde_json::to_string(&json!({"title":name,"price":r["price"],"itemId":r["itemId"]})).map_err(|_|invalid())?,identifier:None,reason:"Authorized eBay image-search candidate; listing similarity does not verify garment identity.".into()});
    }
    Ok(Outcome {
        model: None,
        items: vec![],
        attributes: vec![],
        products,
        sources,
        search_events: vec![json!({"provider":"ebay","type":"search_by_image","total":v["total"]})],
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ebay_identity_is_never_inferred_from_listing_id() {
        let r=parse_ebay(&json!({"itemSummaries":[{"itemId":"v1|123|0","title":"Blue dress","itemWebUrl":"https://www.ebay.com/itm/123","price":{"value":"12.00","currency":"USD"}}]})).unwrap();
        assert_eq!(r.products[0].price.as_deref(), Some("12.00"));
        assert!(r.products[0].identifier.is_none());
        assert!(r.products[0].availability.is_none());
    }
}
