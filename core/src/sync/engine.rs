#![allow(dead_code)]
use futures::{channel::mpsc, SinkExt};
use serde::{Deserialize, Serialize};

use super::{
  crdt::PoMethod, examples::tag::TagCreate, CrdtCtx, FakeCoreContext, PropertyOperation, SyncMethod,
};

pub struct SyncEngine {
  uhlc: uhlc::HLC, // clock
  client_pool_sender: mpsc::Sender<SyncEvent>,
  ctx: SyncContext,
}

#[derive(Clone)]
pub struct SyncContext {
  // pub database: Arc<PrismaClient>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename = "se")]
pub struct SyncEvent {
  #[serde(rename = "u")]
  pub client_uuid: String, // client that created change
  #[serde(rename = "t")]
  pub timestamp: uhlc::Timestamp, // unique hybrid logical clock timestamp
  #[serde(rename = "m")]
  pub method: SyncMethod, // the CRDT resource
  #[serde(rename = "s")]
  pub transport: SyncTransport, // method of data transport
}

impl SyncEvent {
  pub fn new(client_uuid: String, timestamp: uhlc::Timestamp, method: SyncMethod) -> Self {
    Self {
      client_uuid,
      timestamp,
      method,
      transport: SyncTransport::Message,
    }
  }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum SyncTransport {
  Message,
  Binary,
}

impl SyncEngine {
  pub fn new(core_ctx: &FakeCoreContext) -> Self {
    let (client_pool_sender, _client_pool_receiver) = mpsc::channel(10);

    SyncEngine {
      uhlc: uhlc::HLC::default(),
      client_pool_sender,
      ctx: SyncContext {
        // database: core_ctx.database.clone(),
      },
    }
  }

  pub fn exec_event(&mut self, event: SyncEvent) {
    let ctx = self.ctx.clone();
    let time = self.uhlc.update_with_timestamp(&event.timestamp);

    if time.is_err() {
      println!("Time drift detected: {:?}", time);
      return;
    }

    match event.method {
      SyncMethod::PropertyOperation(operation) => PropertyOperation::apply(operation, ctx),
      SyncMethod::Replicate(_) => todo!(),
    }
  }

  pub async fn new_operation(&self, uuid: String, property_operation: PropertyOperation) {
    // create an operation for this resource
    let operation = SyncMethod::PropertyOperation(CrdtCtx {
      uuid: uuid.clone(),
      resource: property_operation,
    });
    // wrap in a sync event
    let event = SyncEvent::new(uuid, self.uhlc.new_timestamp(), operation);

    self.create_sync_event(event).await;
  }

  pub async fn create_sync_event(&self, event: SyncEvent) {
    // let ctx = self.ctx.clone();
    let mut sender = self.client_pool_sender.clone();
    // run locally first

    // if that worked, write sync event to database
    // ctx.database;

    println!("{}", serde_json::to_string_pretty(&event).unwrap());

    // finally send to client pool
    sender.send(event).await.unwrap();
  }
  // pub dn
}

pub async fn test(ctx: &FakeCoreContext) {
  let engine = SyncEngine::new(&ctx);

  let uuid = "12345".to_string();
  let name = "test".to_string();

  engine
    .new_operation(
      uuid,
      PropertyOperation::Tag(PoMethod::Create(TagCreate { name })),
    )
    .await;
}
