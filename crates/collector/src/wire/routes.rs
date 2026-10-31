use super::*;
use crate::http::{Access, HttpShell};
use axum::{
    Json,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message as WsMessage, WebSocket},
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;
pub fn routes(shell: HttpShell, stream: Stream) -> HttpShell {
    shell
        .route("/ws", get(upgrade).with_state(stream.clone()), Access::Any)
        .route(
            "/api/cars/{index}/laps/{lap}/trace",
            get(trace).with_state(stream),
            Access::Any,
        )
}
async fn upgrade(State(stream): State<Stream>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.max_message_size(1024)
        .on_upgrade(move |socket| serve(socket, stream.subscribe()))
}
async fn serve(mut socket: WebSocket, screen: Screen) {
    loop {
        if screen.closed() {
            break;
        }
        if let Some(msg) = screen.pop() {
            let msg = match msg {
                Message::Text(v) => WsMessage::Text(v.to_string().into()),
                Message::Frame(v) => WsMessage::Binary(v.into()),
            };
            // A stalled write must still notice queue overflow promptly.
            let send = socket.send(msg);
            tokio::pin!(send);
            let result = loop {
                tokio::select! {result=&mut send=>break result, _=screen.ready()=>{if screen.closed(){return}}}
            };
            if result.is_err() {
                break;
            }
            continue;
        }
        tokio::select! { _=screen.ready()=>{}, msg=socket.recv()=>match msg {Some(Ok(WsMessage::Close(_)))|None|Some(Err(_))=>break,_=>{}} }
    }
}
#[derive(Deserialize)]
struct Epoch {
    epoch: u32,
}
async fn trace(
    State(stream): State<Stream>,
    Path((index, lap)): Path<(usize, u8)>,
    Query(query): Query<Epoch>,
) -> Result<Json<Trace>, StatusCode> {
    stream
        .trace(index, lap, query.epoch)
        .map(Json)
        .map_err(|s| StatusCode::from_u16(s).unwrap())
}
