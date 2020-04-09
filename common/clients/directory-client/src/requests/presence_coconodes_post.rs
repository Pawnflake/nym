// Copyright 2020 Nym Technologies SA
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::presence::coconodes::CocoPresence;
use reqwest::Response;

pub struct Request {
    base_url: String,
    path: String,
}

pub trait PresenceCocoNodesPoster {
    fn new(base_url: String) -> Self;
    fn post(&self, presence: &CocoPresence) -> Result<Response, reqwest::Error>;
}

impl PresenceCocoNodesPoster for Request {
    fn new(base_url: String) -> Self {
        Request {
            base_url,
            path: "/api/presence/coconodes".to_string(),
        }
    }

    fn post(&self, presence: &CocoPresence) -> Result<Response, reqwest::Error> {
        let url = format!("{}{}", self.base_url, self.path);
        let client = reqwest::Client::new();
        let p = client.post(&url).json(&presence).send()?;
        Ok(p)
    }
}

#[cfg(test)]
mod metrics_get_request {
    use super::*;

    #[cfg(test)]
    use mockito::mock;

    #[cfg(test)]
    mod on_a_400_status {
        use super::*;

        #[test]
        fn it_returns_an_error() {
            let _m = mock("POST", "/api/presence/coconodes")
                .with_status(400)
                .create();
            let req = Request::new(mockito::server_url());
            let presence = fixtures::new_presence();
            let result = req.post(&presence);
            assert_eq!(400, result.unwrap().status());
            _m.assert();
        }
    }

    #[cfg(test)]
    mod on_a_200 {
        use super::*;
        #[test]
        fn it_returns_a_response_with_201() {
            let json = r#"{
                          "ok": true
                      }"#;
            let _m = mock("POST", "/api/presence/coconodes")
                .with_status(201)
                .with_body(json)
                .create();
            let req = Request::new(mockito::server_url());
            let presence = fixtures::new_presence();
            let result = req.post(&presence);
            assert_eq!(true, result.is_ok());
            _m.assert();
        }
    }

    #[cfg(test)]
    mod fixtures {
        use crate::presence::coconodes::CocoPresence;

        pub fn new_presence() -> CocoPresence {
            CocoPresence {
                location: "foomp".to_string(),
                host: "foo.com".to_string(),
                pub_key: "abc".to_string(),
                last_seen: 666,
                version: "0.2.0".to_string(),
            }
        }
    }
}
