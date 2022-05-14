use crate::gettext::gettextf;
use crate::globals;
use crate::globals::TRANSMIT_CODE_FIND_REGEX;
use gettextrs::gettext;
use std::str::FromStr;

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

pub struct WormholeURI {
    pub code: String,
    pub version: usize,
    pub app_id: String,
    pub rendezvous_server: String,
    pub direction: TransferDirection,
}

impl WormholeURI {
    pub fn new(code: &str) -> Self {
        Self {
            code: code.to_string(),
            version: 0,
            app_id: globals::WORMHOLE_DEFAULT_APPID_STR.to_string(),
            rendezvous_server: globals::WORMHOLE_DEFAULT_RENDEZVOUS_SERVER.to_string(),
            direction: TransferDirection::Receive,
        }
    }

    pub fn create_uri(&self) -> String {
        let mut uri =
            url::Url::parse(&format!("wormhole:{}", urlencoding::encode(&self.code))).unwrap();

        if self.version != 0 {
            uri.query_pairs_mut()
                .append_pair("version", &self.version.to_string());
        }

        if self.app_id != globals::WORMHOLE_DEFAULT_APPID_STR {
            uri.query_pairs_mut().append_pair("appid", &self.app_id);
        }

        if self.rendezvous_server != globals::WORMHOLE_DEFAULT_RENDEZVOUS_SERVER {
            uri.query_pairs_mut()
                .append_pair("rendezvous", &self.rendezvous_server);
        }

        if self.direction != TransferDirection::Receive {
            uri.query_pairs_mut().append_pair("type", "send");
        }

        uri.to_string()
    }
}

impl TryFrom<url::Url> for WormholeURI {
    type Error = WormholeURIParseError;

    fn try_from(uri: url::Url) -> Result<Self, Self::Error> {
        // Basic validation
        if uri.scheme() != "wormhole" || uri.has_host() || uri.has_authority() || uri.path() == "" {
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

        let mut this = WormholeURI::new(&code);

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
                "appid" => this.app_id = value.to_string(),
                "rendezvous" => this.rendezvous_server = value.to_string(),
                "type" => {
                    this.direction = if value == "recv" {
                        TransferDirection::Receive
                    } else if value == "send" {
                        TransferDirection::Send
                    } else {
                        return Err(WormholeURIParseError(gettextf(
                            "The URI parameter 'type' must be 'recv' or 'send' (was: {})",
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

impl FromStr for WormholeURI {
    type Err = WormholeURIParseError;

    fn from_str(uri_str: &str) -> Result<Self, Self::Err> {
        let uri = url::Url::parse(uri_str)
            .map_err(|_| WormholeURIParseError(gettext("The URI format is invalid")))?;
        Self::try_from(uri)
    }
}

#[cfg(test)]
mod test {
    use crate::util::{TransferDirection, WormholeURI};

    #[test]
    fn test_create_uri() {
        let params1 = WormholeURI::new("4-hurricane-equipment");
        assert_eq!(params1.create_uri(), "wormhole:4-hurricane-equipment");

        let params2 = WormholeURI::new("8-🙈-🙉-🙊");
        assert_eq!(
            params2.create_uri(),
            "wormhole:8-%F0%9F%99%88-%F0%9F%99%89-%F0%9F%99%8A"
        );

        let mut params3 = WormholeURI::new("8-🙈-🙉-🙊");
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

        let parsed_params3 = params3.create_uri().parse::<WormholeURI>().unwrap();
        assert_eq!(params3.app_id, parsed_params3.app_id);
        assert_eq!(params3.rendezvous_server, parsed_params3.rendezvous_server);
        assert_eq!(params3.version, parsed_params3.version);
        assert_eq!(params3.direction, parsed_params3.direction);
    }
}
