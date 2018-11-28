use crate::types::NodeAppConfig;
use actix::prelude::*;
use actix::Message;
use serde_derive::{Deserialize, Serialize};
use sodiumoxide::crypto::box_ as cipher;
use sodiumoxide::crypto::sign::ed25519::{PublicKey, Signature};
use std::collections::HashMap;
use std::time::SystemTime;

use crate::codec;
use crate::session;
use crate::sign;

/// `MqServer` manages MQ network and
/// responsible for network nodes
/// coordinating.
pub struct MqServer {
    sessions: HashMap<PublicKey, Addr<session::MqSession>>,
    settigns: NodeAppConfig,
}

impl MqServer {
    pub fn new(cfg: NodeAppConfig) -> MqServer {
        MqServer {
            sessions: HashMap::new(),
            settigns: cfg,
        }
    }
}

/// Make actor from `MqServer`
impl Actor for MqServer {
    /// We are going to use simple Context, we just need ability to communicate
    /// with other actors.
    type Context = Context<Self>;
}

/// Message for MQ server communications

/// New MQ session is created
pub struct Connect {
    pub addr: Addr<session::MqSession>,
}

/// Response type for Connect message
///
/// MQ server returns unique session id
impl actix::Message for Connect {
    type Result = PublicKey;
}

/// Session is disconnected
#[derive(Message)]
pub struct Disconnect(pub PublicKey);

/// Basic MQ Message Data
#[derive(Message, Debug, Deserialize, Serialize, Clone)]
pub struct MqMessage {
    pub from: PublicKey,
    pub to: PublicKey,
    pub signature: Option<Signature>,
    pub name: Option<String>,
    pub protocol: codec::MessageProtocol,
    pub time: SystemTime,
    pub nonce: Option<cipher::Nonce>,
    pub body: String,
}

impl MqMessage {
    /// Convert message to Client message
    fn to_message(&self) -> codec::MessageData {
        codec::MessageData {
            to: self.to,
            signature: self.signature,
            name: self.name.clone(),
            protocol: self.protocol.clone(),
            time: self.time,
            nonce: self.nonce,
            body: self.body.clone(),
        }
    }
}

/// Ping message for specific client
#[derive(Message)]
pub struct MqPingClient {
    pub from: PublicKey,
    pub to: PublicKey,
}

/// Pong message for specific client
#[derive(Message)]
pub struct MqPongClient {
    pub from: PublicKey,
    pub to: PublicKey,
}

/// Register client
pub struct MqRegister {
    /// Old client identifier
    /// as temporary value that should
    /// be set to real client pub_key
    pub old_pub_key: PublicKey,
    /// Client identifier
    pub pub_key: PublicKey,
}

/// Response type for Register message
/// It can be success or fail
impl actix::Message for MqRegister {
    type Result = Option<PublicKey>;
}

/// Handler for Connect message.
///
/// Register new session and assign unique id to this session
impl Handler<Connect> for MqServer {
    type Result = MessageResult<Connect>;

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        println!("Handler<Connect>");
        // Generate temporary pub_key for session identification
        let (pk, _) = sign::gen_keypair();
        self.sessions.insert(pk, msg.addr);
        MessageResult(pk)
    }
}

/// Handler for Disconnect message.
impl Handler<Disconnect> for MqServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        let pub_key = msg.0;
        println!("Handler<Disconnect>");
        // Unregister session
        self.sessions.remove(&pub_key);
    }
}

/// Handler for Message message.
impl Handler<MqMessage> for MqServer {
    type Result = ();

    fn handle(&mut self, msg: MqMessage, _: &mut Context<Self>) {
        println!("Handler<Message>");
        if let Some(addr) = self.sessions.get(&msg.to) {
            addr.do_send(session::MqSessionMessage(msg))
        }
    }
}

/// Handler for Register message.
impl Handler<MqRegister> for MqServer {
    type Result = MessageResult<MqRegister>;

    fn handle(&mut self, msg: MqRegister, _: &mut Context<Self>) -> Self::Result {
        println!("Handler<Register>");

        // Check is Client already registered
        if self.sessions.get(&msg.pub_key).is_some() {
            eprintln!("Client already registered - close session");
            return MessageResult(None);
        }

        if let Some(addr) = self.sessions.get(&msg.old_pub_key) {
            self.sessions.insert(msg.pub_key, addr.to_owned());
            self.sessions.remove(&msg.old_pub_key);
        } else {
            eprintln!("Session address not found");
            return MessageResult(None);
        }
        MessageResult(Some(msg.pub_key))
    }
}

/// Handler for Ping Client message.
impl Handler<MqPingClient> for MqServer {
    type Result = ();

    fn handle(&mut self, msg: MqPingClient, _: &mut Context<Self>) -> Self::Result {
        println!("Handler<MqPingClient>");

        if let Some(addr) = self.sessions.get(&msg.to) {
            addr.do_send(session::MqSessionPingClient(msg.from));
        }
    }
}

/// Handler for Pong Client message.
impl Handler<MqPongClient> for MqServer {
    type Result = ();

    fn handle(&mut self, msg: MqPongClient, _: &mut Context<Self>) -> Self::Result {
        println!("Handler<MqPongClient>");

        if let Some(addr) = self.sessions.get(&msg.to) {
            addr.do_send(session::MqSessionPongClient(msg.from));
        }
    }
}
