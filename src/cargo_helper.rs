use ansi_term::Colour::{Green, Yellow, White};
use authorization::{AuthorizationHeaderFactory, AuthorizationType, GetAuthorizationType};
use Bytes;
use client::GetResponse;
use contentlength::GetContentLength;
use http_version::ValidateHttpVersion;
use hyper::header::{ByteRangeSpec, Headers, Range};
use hyper::client::Client;
use response::CheckResponseStatus;
use std::result::Result;
use util::prompt_user;

pub struct CargoInfo {
    pub accept_partialcontent: bool,
    pub auth_header: Option<AuthorizationHeaderFactory>,
    pub content_length: Bytes,
}

pub fn get_cargo_info(url: &str) -> Result<CargoInfo, String> {
    let hyper_client = Client::new();

    let client_response = hyper_client.get_head_response(url).unwrap();

    print!("# Waiting a response from the remote server... ");

    if !client_response.version.greater_than_http_11() {
        println!("{}",
                 Yellow.bold().paint("OK (HTTP version <= 1.0 detected)"));
    } else {
        println!("{}", Green.bold().paint("OK !"));
    }

    let auth_type = client_response.headers.get_authorization_type();
    let auth_header_factory = match auth_type {
        Some(a_type) => {
            match a_type {
                AuthorizationType::Basic => {
                    println!("{}",
                             Yellow
                                 .bold()
                                 .paint("[WARNING] The remote content is protected by Basic \
                                         Auth.\nPlease to enter below your credential \
                                         informations."));
                    let username = prompt_user(White.bold(), "Username:");
                    let password = prompt_user(White.bold(), "Password:");
                    Some(AuthorizationHeaderFactory::new(AuthorizationType::Basic,
                                                         username,
                                                         Some(password)))
                }
                _ => {
                    return Err(format!("[ERROR] The remote content is protected by {} \
                                                 Authorization, which is not supported!\nYou \
                                                 can create a new issue to report this problem \
                                                 in https://github.\
                                                 com/derniercri/snatch/issues/new",
                                       a_type));
                }
            }
        }
        None => None,
    };

    let client_response = match auth_header_factory.clone() {
        Some(header_factory) => {
            let mut headers = Headers::new();
            headers.set(header_factory.build_header());
            hyper_client
                .get_head_response_using_headers(&url, headers)
                .unwrap()
        }
        None => client_response,
    };

    let remote_content_length = match client_response.headers.get_content_length() {
        Some(remote_content_length) => remote_content_length,
        None => {

            println!("{}",
                     Yellow
                         .bold()
                         .paint("[WARNING] Cannot get the remote content length, using an \
                                 HEADER request."));
            println!("{}",
                     Yellow
                         .bold()
                         .paint("[WARNING] Trying to send an HTTP request, to get the remote \
                                 content length..."));

            // Trying to force the server to send to us the remote content length
            let mut custom_http_header = Headers::new();
            // HTTP header to get all the remote content - if the response is OK, get the
            // ContentLength information sent back from the server
            custom_http_header.set(Range::Bytes(vec![ByteRangeSpec::AllFrom(0)]));
            // Get a response from the server, using the custom HTTP request
            let client_response = hyper_client
                .get_http_response_using_headers(&url, custom_http_header)
                .unwrap();
            // Try again to get the content length - if this one is unknown again, stop the program
            match client_response.headers.get_content_length() {
                Some(remote_content_length) => remote_content_length,
                None => {
                    return Err("[ERROR] Second attempt has failed.".to_string());
                }
            }
        }
    };

    // Ask the first byte, just to know if the server accept PartialContent status
    let mut header = Headers::new();
    header.set(Range::Bytes(vec![ByteRangeSpec::FromTo(0, 1)]));

    let client_response = hyper_client
        .get_head_response_using_headers(url, header)
        .unwrap();

    print!("# Checking the server's support for PartialContent headers...");

    Ok(CargoInfo {
           accept_partialcontent: client_response.check_partialcontent_status(),
           auth_header: auth_header_factory,
           content_length: remote_content_length,
       })
}
