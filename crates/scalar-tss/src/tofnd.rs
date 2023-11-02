/// Key presence check types
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeyPresenceRequest {
    #[prost(string, tag = "1")]
    pub key_uid: ::prost::alloc::string::String,
    /// SEC1-encoded compressed pub key bytes to find the right mnemonic. Latest is used, if empty.
    #[prost(bytes = "vec", tag = "2")]
    pub pub_key: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeyPresenceResponse {
    #[prost(enumeration = "key_presence_response::Response", tag = "1")]
    pub response: i32,
}
/// Nested message and enum types in `KeyPresenceResponse`.
pub mod key_presence_response {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Response {
        Unspecified = 0,
        Present = 1,
        Absent = 2,
        Fail = 3,
    }
    impl Response {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Response::Unspecified => "RESPONSE_UNSPECIFIED",
                Response::Present => "RESPONSE_PRESENT",
                Response::Absent => "RESPONSE_ABSENT",
                Response::Fail => "RESPONSE_FAIL",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "RESPONSE_UNSPECIFIED" => Some(Self::Unspecified),
                "RESPONSE_PRESENT" => Some(Self::Present),
                "RESPONSE_ABSENT" => Some(Self::Absent),
                "RESPONSE_FAIL" => Some(Self::Fail),
                _ => None,
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RecoverRequest {
    #[prost(message, optional, tag = "1")]
    pub keygen_init: ::core::option::Option<KeygenInit>,
    #[prost(message, optional, tag = "2")]
    pub keygen_output: ::core::option::Option<KeygenOutput>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RecoverResponse {
    #[prost(enumeration = "recover_response::Response", tag = "1")]
    pub response: i32,
}
/// Nested message and enum types in `RecoverResponse`.
pub mod recover_response {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Response {
        Unspecified = 0,
        Success = 1,
        Fail = 2,
    }
    impl Response {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Response::Unspecified => "RESPONSE_UNSPECIFIED",
                Response::Success => "RESPONSE_SUCCESS",
                Response::Fail => "RESPONSE_FAIL",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "RESPONSE_UNSPECIFIED" => Some(Self::Unspecified),
                "RESPONSE_SUCCESS" => Some(Self::Success),
                "RESPONSE_FAIL" => Some(Self::Fail),
                _ => None,
            }
        }
    }
}
/// Keygen's success response
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeygenOutput {
    /// pub_key; common for all parties
    #[prost(bytes = "vec", tag = "1")]
    pub pub_key: ::prost::alloc::vec::Vec<u8>,
    /// recovery info common for all parties
    #[prost(bytes = "vec", tag = "2")]
    pub group_recover_info: ::prost::alloc::vec::Vec<u8>,
    /// recovery info unique for each party
    #[prost(bytes = "vec", tag = "3")]
    pub private_recover_info: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MessageIn {
    /// TODO don't reuse `data`
    #[prost(oneof = "message_in::Data", tags = "1, 2, 3, 4")]
    pub data: ::core::option::Option<message_in::Data>,
}
/// Nested message and enum types in `MessageIn`.
pub mod message_in {
    /// TODO don't reuse `data`
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Data {
        /// first message only, Keygen
        #[prost(message, tag = "1")]
        KeygenInit(super::KeygenInit),
        /// first message only, Sign
        #[prost(message, tag = "2")]
        SignInit(super::SignInit),
        /// all subsequent messages
        #[prost(message, tag = "3")]
        Traffic(super::TrafficIn),
        /// abort the protocol, ignore the bool value
        #[prost(bool, tag = "4")]
        Abort(bool),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MessageOut {
    /// TODO don't reuse `data`
    #[prost(oneof = "message_out::Data", tags = "1, 2, 3, 4")]
    pub data: ::core::option::Option<message_out::Data>,
}
/// Nested message and enum types in `MessageOut`.
pub mod message_out {
    /// Keygen's response types
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct KeygenResult {
        #[prost(oneof = "keygen_result::KeygenResultData", tags = "1, 2")]
        pub keygen_result_data: ::core::option::Option<keygen_result::KeygenResultData>,
    }
    /// Nested message and enum types in `KeygenResult`.
    pub mod keygen_result {
        #[allow(clippy::derive_partial_eq_without_eq)]
        #[derive(Clone, PartialEq, ::prost::Oneof)]
        pub enum KeygenResultData {
            /// Success response
            #[prost(message, tag = "1")]
            Data(super::super::KeygenOutput),
            /// Failure response
            #[prost(message, tag = "2")]
            Criminals(super::CriminalList),
        }
    }
    /// Sign's response types
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct SignResult {
        #[prost(oneof = "sign_result::SignResultData", tags = "1, 2")]
        pub sign_result_data: ::core::option::Option<sign_result::SignResultData>,
    }
    /// Nested message and enum types in `SignResult`.
    pub mod sign_result {
        #[allow(clippy::derive_partial_eq_without_eq)]
        #[derive(Clone, PartialEq, ::prost::Oneof)]
        pub enum SignResultData {
            /// Success response
            #[prost(bytes, tag = "1")]
            Signature(::prost::alloc::vec::Vec<u8>),
            /// Failure response
            #[prost(message, tag = "2")]
            Criminals(super::CriminalList),
        }
    }
    /// Keygen/Sign failure response message
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct CriminalList {
        #[prost(message, repeated, tag = "1")]
        pub criminals: ::prost::alloc::vec::Vec<criminal_list::Criminal>,
    }
    /// Nested message and enum types in `CriminalList`.
    pub mod criminal_list {
        #[allow(clippy::derive_partial_eq_without_eq)]
        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct Criminal {
            #[prost(string, tag = "1")]
            pub party_uid: ::prost::alloc::string::String,
            #[prost(enumeration = "criminal::CrimeType", tag = "2")]
            pub crime_type: i32,
        }
        /// Nested message and enum types in `Criminal`.
        pub mod criminal {
            #[derive(
                Clone,
                Copy,
                Debug,
                PartialEq,
                Eq,
                Hash,
                PartialOrd,
                Ord,
                ::prost::Enumeration
            )]
            #[repr(i32)]
            pub enum CrimeType {
                Unspecified = 0,
                NonMalicious = 1,
                Malicious = 2,
            }
            impl CrimeType {
                /// String value of the enum field names used in the ProtoBuf definition.
                ///
                /// The values are not transformed in any way and thus are considered stable
                /// (if the ProtoBuf definition does not change) and safe for programmatic use.
                pub fn as_str_name(&self) -> &'static str {
                    match self {
                        CrimeType::Unspecified => "CRIME_TYPE_UNSPECIFIED",
                        CrimeType::NonMalicious => "CRIME_TYPE_NON_MALICIOUS",
                        CrimeType::Malicious => "CRIME_TYPE_MALICIOUS",
                    }
                }
                /// Creates an enum from field names used in the ProtoBuf definition.
                pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
                    match value {
                        "CRIME_TYPE_UNSPECIFIED" => Some(Self::Unspecified),
                        "CRIME_TYPE_NON_MALICIOUS" => Some(Self::NonMalicious),
                        "CRIME_TYPE_MALICIOUS" => Some(Self::Malicious),
                        _ => None,
                    }
                }
            }
        }
    }
    /// TODO don't reuse `data`
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Data {
        /// all but final message
        #[prost(message, tag = "1")]
        Traffic(super::TrafficOut),
        /// final message only, Keygen
        #[prost(message, tag = "2")]
        KeygenResult(KeygenResult),
        /// final message only, Sign
        #[prost(message, tag = "3")]
        SignResult(SignResult),
        /// issue recover from client
        #[prost(bool, tag = "4")]
        NeedRecover(bool),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TrafficIn {
    #[prost(string, tag = "1")]
    pub from_party_uid: ::prost::alloc::string::String,
    #[prost(bytes = "vec", tag = "2")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
    #[prost(bool, tag = "3")]
    pub is_broadcast: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TrafficOut {
    #[prost(string, tag = "1")]
    pub to_party_uid: ::prost::alloc::string::String,
    #[prost(bytes = "vec", tag = "2")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
    #[prost(bool, tag = "3")]
    pub is_broadcast: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeygenInit {
    #[prost(string, tag = "1")]
    pub new_key_uid: ::prost::alloc::string::String,
    #[prost(string, repeated, tag = "2")]
    pub party_uids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(uint32, repeated, tag = "5")]
    pub party_share_counts: ::prost::alloc::vec::Vec<u32>,
    /// parties\[my_party_index\] belongs to the server
    #[prost(uint32, tag = "3")]
    pub my_party_index: u32,
    #[prost(uint32, tag = "4")]
    pub threshold: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SignInit {
    #[prost(string, tag = "1")]
    pub new_sig_uid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub key_uid: ::prost::alloc::string::String,
    /// TODO replace this with a subset of indices?
    #[prost(string, repeated, tag = "3")]
    pub party_uids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(bytes = "vec", tag = "4")]
    pub message_to_sign: ::prost::alloc::vec::Vec<u8>,
}
/// Generated client implementations.
pub mod gg20_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// GG20 is the protocol https://eprint.iacr.org/2020/540
    /// rpc definitions intended to wrap the API for this library: https://github.com/axelarnetwork/tofn
    #[derive(Debug, Clone)]
    pub struct Gg20Client<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl Gg20Client<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> Gg20Client<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> Gg20Client<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            Gg20Client::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        pub async fn recover(
            &mut self,
            request: impl tonic::IntoRequest<super::RecoverRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RecoverResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/tofnd.GG20/Recover");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tofnd.GG20", "Recover"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn keygen(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::MessageIn>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::MessageOut>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/tofnd.GG20/Keygen");
            let mut req = request.into_streaming_request();
            req.extensions_mut().insert(GrpcMethod::new("tofnd.GG20", "Keygen"));
            self.inner.streaming(req, path, codec).await
        }
        pub async fn sign(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::MessageIn>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::MessageOut>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/tofnd.GG20/Sign");
            let mut req = request.into_streaming_request();
            req.extensions_mut().insert(GrpcMethod::new("tofnd.GG20", "Sign"));
            self.inner.streaming(req, path, codec).await
        }
        pub async fn key_presence(
            &mut self,
            request: impl tonic::IntoRequest<super::KeyPresenceRequest>,
        ) -> std::result::Result<
            tonic::Response<super::KeyPresenceResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/tofnd.GG20/KeyPresence");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("tofnd.GG20", "KeyPresence"));
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod gg20_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with Gg20Server.
    #[async_trait]
    pub trait Gg20: Send + Sync + 'static {
        async fn recover(
            &self,
            request: tonic::Request<super::RecoverRequest>,
        ) -> std::result::Result<tonic::Response<super::RecoverResponse>, tonic::Status>;
        /// Server streaming response type for the Keygen method.
        type KeygenStream: futures_core::Stream<
                Item = std::result::Result<super::MessageOut, tonic::Status>,
            >
            + Send
            + 'static;
        async fn keygen(
            &self,
            request: tonic::Request<tonic::Streaming<super::MessageIn>>,
        ) -> std::result::Result<tonic::Response<Self::KeygenStream>, tonic::Status>;
        /// Server streaming response type for the Sign method.
        type SignStream: futures_core::Stream<
                Item = std::result::Result<super::MessageOut, tonic::Status>,
            >
            + Send
            + 'static;
        async fn sign(
            &self,
            request: tonic::Request<tonic::Streaming<super::MessageIn>>,
        ) -> std::result::Result<tonic::Response<Self::SignStream>, tonic::Status>;
        async fn key_presence(
            &self,
            request: tonic::Request<super::KeyPresenceRequest>,
        ) -> std::result::Result<
            tonic::Response<super::KeyPresenceResponse>,
            tonic::Status,
        >;
    }
    /// GG20 is the protocol https://eprint.iacr.org/2020/540
    /// rpc definitions intended to wrap the API for this library: https://github.com/axelarnetwork/tofn
    #[derive(Debug)]
    pub struct Gg20Server<T: Gg20> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: Gg20> Gg20Server<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for Gg20Server<T>
    where
        T: Gg20,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/tofnd.GG20/Recover" => {
                    #[allow(non_camel_case_types)]
                    struct RecoverSvc<T: Gg20>(pub Arc<T>);
                    impl<T: Gg20> tonic::server::UnaryService<super::RecoverRequest>
                    for RecoverSvc<T> {
                        type Response = super::RecoverResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::RecoverRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).recover(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = RecoverSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/tofnd.GG20/Keygen" => {
                    #[allow(non_camel_case_types)]
                    struct KeygenSvc<T: Gg20>(pub Arc<T>);
                    impl<T: Gg20> tonic::server::StreamingService<super::MessageIn>
                    for KeygenSvc<T> {
                        type Response = super::MessageOut;
                        type ResponseStream = T::KeygenStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<tonic::Streaming<super::MessageIn>>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).keygen(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = KeygenSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/tofnd.GG20/Sign" => {
                    #[allow(non_camel_case_types)]
                    struct SignSvc<T: Gg20>(pub Arc<T>);
                    impl<T: Gg20> tonic::server::StreamingService<super::MessageIn>
                    for SignSvc<T> {
                        type Response = super::MessageOut;
                        type ResponseStream = T::SignStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<tonic::Streaming<super::MessageIn>>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).sign(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = SignSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/tofnd.GG20/KeyPresence" => {
                    #[allow(non_camel_case_types)]
                    struct KeyPresenceSvc<T: Gg20>(pub Arc<T>);
                    impl<T: Gg20> tonic::server::UnaryService<super::KeyPresenceRequest>
                    for KeyPresenceSvc<T> {
                        type Response = super::KeyPresenceResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::KeyPresenceRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).key_presence(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = KeyPresenceSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", "12")
                                .header("content-type", "application/grpc")
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: Gg20> Clone for Gg20Server<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    impl<T: Gg20> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: Gg20> tonic::server::NamedService for Gg20Server<T> {
        const NAME: &'static str = "tofnd.GG20";
    }
}
