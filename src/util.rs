use crate::gettext::gettextf;
use crate::glib::Cast;
use crate::globals;
use crate::globals::TRANSMIT_CODE_FIND_REGEX;
use crate::ui::window::WarpApplicationWindow;
use gettextrs::gettext;
use gtk::gdk;
use qrcode::QrCode;
use std::str::FromStr;
use wormhole::transfer::AppVersion;
use wormhole::{AppConfig, AppID, Code};

pub mod error;
pub mod future;

pub fn extract_transmit_code(str: &str) -> Option<String> {
    TRANSMIT_CODE_FIND_REGEX
        .find(str)
        .map(|m| m.as_str().to_string())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransferDirection {
    Send,
    Receive,
}

impl Default for TransferDirection {
    fn default() -> Self {
        Self::Send
    }
}

#[derive(Debug)]
pub struct WormholeURIParseError(String);

impl ToString for WormholeURIParseError {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

#[derive(Debug)]
pub struct WormholeTransferURI {
    pub code: Code,
    pub version: usize,
    pub rendezvous_server: String,
    pub direction: TransferDirection,
}

impl WormholeTransferURI {
    pub fn new(code: &str) -> Self {
        let rendezvous_server = WarpApplicationWindow::default()
            .config()
            .rendezvous_server_url_or_default()
            .to_string();

        Self {
            code: Code(code.to_string()),
            version: 0,
            rendezvous_server,
            direction: TransferDirection::Receive,
        }
    }

    pub fn create_uri(&self) -> String {
        let mut uri = url::Url::parse(&format!(
            "wormhole-transfer:{}",
            urlencoding::encode(&self.code)
        ))
        .unwrap();

        if self.version != 0 {
            uri.query_pairs_mut()
                .append_pair("version", &self.version.to_string());
        }

        // We take the default here, not the current config. Any non-default should be in the uri.
        if self.rendezvous_server != globals::WORMHOLE_DEFAULT_RENDEZVOUS_SERVER {
            uri.query_pairs_mut()
                .append_pair("rendezvous", &self.rendezvous_server);
        }

        if self.direction != TransferDirection::Receive {
            uri.query_pairs_mut().append_pair("role", "leader");
        }

        uri.to_string()
    }

    pub fn from_app_cfg_with_code_direction(
        app_cfg: &AppConfig<AppVersion>,
        code: &str,
        direction: TransferDirection,
    ) -> Self {
        let rendezvous_server = app_cfg.rendezvous_url.trim_end_matches("/v1").to_string();
        Self {
            code: Code(code.to_string()),
            version: 0,
            rendezvous_server,
            direction,
        }
    }

    pub fn to_app_cfg(&self) -> AppConfig<AppVersion> {
        AppConfig {
            id: AppID::new(globals::WORMHOLE_DEFAULT_APPID_STR),
            rendezvous_url: format!("{}/v1", self.rendezvous_server).into(),
            app_version: AppVersion {},
        }
    }

    pub fn to_paintable_qr(&self) -> gdk::Paintable {
        let qr = QrCode::new(self.create_uri()).unwrap();
        let svg = qr
            .render::<qrcode::render::svg::Color>()
            .min_dimensions(800, 800)
            .build();
        gdk::Texture::from_bytes(&svg.as_bytes().into())
            .unwrap()
            .upcast()
    }
}

impl TryFrom<url::Url> for WormholeTransferURI {
    type Error = WormholeURIParseError;

    fn try_from(uri: url::Url) -> Result<Self, Self::Error> {
        // Basic validation
        if uri.scheme() != "wormhole-transfer"
            || uri.has_host()
            || uri.has_authority()
            || uri.path() == ""
        {
            return Err(WormholeURIParseError(gettext("The URI format is invalid")));
        }

        let code = urlencoding::decode(uri.path()).map_err(|_| {
            WormholeURIParseError(gettext("The code does not match the required format"))
        })?;
        if !globals::TRANSMIT_CODE_MATCH_REGEX.is_match(&code) {
            return Err(WormholeURIParseError(gettext(
                "The code does not match the required format",
            )));
        }

        let mut this = WormholeTransferURI::new(&code);

        for (field, value) in uri.query_pairs() {
            match &*field {
                "version" => {
                    this.version = {
                        let value_num = value.parse().map_err(|_| {
                            WormholeURIParseError(gettextf("Unknown URI version: {}", &[&value]))
                        })?;
                        if value_num == 0 {
                            value_num
                        } else {
                            return Err(WormholeURIParseError(gettextf(
                                "Unknown URI version: {}",
                                &[&value],
                            )));
                        }
                    }
                }
                "rendezvous" => this.rendezvous_server = value.to_string(),
                "role" => {
                    this.direction = if value == "follower" {
                        TransferDirection::Receive
                    } else if value == "leader" {
                        TransferDirection::Send
                    } else {
                        return Err(WormholeURIParseError(gettextf(
                            "The URI parameter 'role' must be 'follower' or 'leader' (was: {})",
                            &[&value],
                        )));
                    }
                }
                _ => {
                    return Err(WormholeURIParseError(gettextf(
                        "Unknown URI parameter '{}'",
                        &[&field],
                    )))
                }
            }
        }

        Ok(this)
    }
}

impl FromStr for WormholeTransferURI {
    type Err = WormholeURIParseError;

    fn from_str(uri_str: &str) -> Result<Self, Self::Err> {
        let uri = url::Url::parse(uri_str)
            .map_err(|_| WormholeURIParseError(gettext("The URI format is invalid")))?;
        Self::try_from(uri)
    }
}

#[cfg(test)]
mod test {
    use crate::util::{TransferDirection, WormholeTransferURI};

    #[test]
    fn test_create_uri() {
        let params1 = WormholeTransferURI::new("4-hurricane-equipment");
        assert_eq!(params1.create_uri(), "wormhole:4-hurricane-equipment");

        let params2 = WormholeTransferURI::new("8-🙈-🙉-🙊");
        assert_eq!(
            params2.create_uri(),
            "wormhole:8-%F0%9F%99%88-%F0%9F%99%89-%F0%9F%99%8A"
        );

        let mut params3 = WormholeTransferURI::new("8-🙈-🙉-🙊");
        params3.app_id = "test-appid".to_string();
        params3.rendezvous_server = "ws://localhost:4000".to_string();
        params3.version = 1;
        params3.direction = TransferDirection::Send;

        assert_eq!(
            params3.create_uri(),
            "wormhole:8-%F0%9F%99%88-%F0%9F%99%89-%F0%9F%99%8A?version=1&appid=test-appid&rendezvous=ws%3A%2F%2Flocalhost%3A4000&type=send"
        );

        // Version != 0 would result in parse error
        params3.version = 0;

        let parsed_params3 = params3.create_uri().parse::<WormholeTransferURI>().unwrap();
        assert_eq!(params3.app_id, parsed_params3.app_id);
        assert_eq!(params3.rendezvous_server, parsed_params3.rendezvous_server);
        assert_eq!(params3.version, parsed_params3.version);
        assert_eq!(params3.direction, parsed_params3.direction);
    }
}
