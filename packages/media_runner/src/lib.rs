mod worker;

use media_server_protocol::protobuf::cluster_media::{
    media_edge_server::MediaEdge, RtpEngineCreateAnswerRequest, RtpEngineCreateAnswerResponse, RtpEngineCreateOfferRequest, RtpEngineCreateOfferResponse, RtpEngineDeleteRequest,
    RtpEngineDeleteResponse, RtpEngineSetAnswerRequest, RtpEngineSetAnswerResponse, WebrtcConnectRequest, WebrtcConnectResponse, WebrtcRemoteIceRequest, WebrtcRemoteIceResponse,
    WebrtcRestartIceRequest, WebrtcRestartIceResponse, WhepCloseRequest, WhepCloseResponse, WhepConnectRequest, WhepConnectResponse, WhepRemoteIceRequest, WhepRemoteIceResponse, WhipCloseRequest,
    WhipCloseResponse, WhipConnectRequest, WhipConnectResponse, WhipRemoteIceRequest, WhipRemoteIceResponse,
};
use tonic::{Request, Response, Status};
pub use worker::{Input, MediaConfig, MediaServerWorker, Output, Owner, SdnConfig, UserData, SC, SE, TC, TW};

pub struct MediaRunnerService {}

#[async_trait::async_trait]
impl MediaEdge for MediaRunnerService {
    async fn whip_connect(&self, request: Request<WhipConnectRequest>) -> std::result::Result<Response<WhipConnectResponse>, Status> {
        todo!()
    }
    async fn whip_remote_ice(&self, request: Request<WhipRemoteIceRequest>) -> std::result::Result<Response<WhipRemoteIceResponse>, Status> {
        todo!()
    }
    async fn whip_close(&self, request: Request<WhipCloseRequest>) -> std::result::Result<Response<WhipCloseResponse>, Status> {
        todo!()
    }
    async fn whep_connect(&self, request: Request<WhepConnectRequest>) -> std::result::Result<Response<WhepConnectResponse>, Status> {
        todo!()
    }
    async fn whep_remote_ice(&self, request: Request<WhepRemoteIceRequest>) -> std::result::Result<Response<WhepRemoteIceResponse>, Status> {
        todo!()
    }
    async fn whep_close(&self, request: Request<WhepCloseRequest>) -> std::result::Result<Response<WhepCloseResponse>, Status> {
        todo!()
    }
    async fn webrtc_connect(&self, request: Request<WebrtcConnectRequest>) -> std::result::Result<Response<WebrtcConnectResponse>, Status> {
        todo!()
    }
    async fn webrtc_remote_ice(&self, request: Request<WebrtcRemoteIceRequest>) -> std::result::Result<Response<WebrtcRemoteIceResponse>, Status> {
        todo!()
    }
    async fn webrtc_restart_ice(&self, request: Request<WebrtcRestartIceRequest>) -> std::result::Result<Response<WebrtcRestartIceResponse>, Status> {
        todo!()
    }
    async fn rtp_engine_create_offer(&self, request: Request<RtpEngineCreateOfferRequest>) -> std::result::Result<Response<RtpEngineCreateOfferResponse>, Status> {
        todo!()
    }
    async fn rtp_engine_set_answer(&self, request: Request<RtpEngineSetAnswerRequest>) -> std::result::Result<Response<RtpEngineSetAnswerResponse>, Status> {
        todo!()
    }
    async fn rtp_engine_create_answer(&self, request: Request<RtpEngineCreateAnswerRequest>) -> std::result::Result<Response<RtpEngineCreateAnswerResponse>, Status> {
        todo!()
    }
    async fn rtp_engine_delete(&self, request: Request<RtpEngineDeleteRequest>) -> std::result::Result<Response<RtpEngineDeleteResponse>, Status> {
        todo!()
    }
}
