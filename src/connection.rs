use futures_util::{SinkExt, StreamExt};
use log::debug;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tungstenite::Message;

pub(crate) struct WsClient {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl WsClient {
    pub(crate) async fn connect_to(path: &str) -> WsClient {
        let (socket, _) = connect_async(path).await.expect("");
        WsClient { socket }
    }

    pub(crate) async fn send(&mut self, s: String) {
        debug!("Send to ws: {}", s);
        self.socket.send(Message::Text(s)).await.unwrap();
    }

    pub(crate) async fn wait(&mut self) -> Option<String> {
        match self.socket.next().await {
            Some(msg) => msg.ok().map(|x| x.into_text().unwrap()),
            None => None,
        }
    }
}
