// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::state::State;
use deno_core::BufVec;
use deno_core::ErrBox;
use deno_core::OpRegistry;
use deno_core::ZeroCopyBuf;
use serde_json::Value;
use std::rc::Rc;

#[cfg(unix)]
use futures::future::poll_fn;
#[cfg(unix)]
use serde_derive::Deserialize;
#[cfg(unix)]
use std::task::Waker;
#[cfg(unix)]
use tokio::signal::unix::{signal, Signal, SignalKind};

pub fn init(s: &Rc<State>) {
  s.register_op_json_sync("op_signal_bind", op_signal_bind);
  s.register_op_json_sync("op_signal_unbind", op_signal_unbind);
  s.register_op_json_async("op_signal_poll", op_signal_poll);
}

#[cfg(unix)]
/// The resource for signal stream.
/// The second element is the waker of polling future.
pub struct SignalStreamResource(pub Signal, pub Option<Waker>);

#[cfg(unix)]
#[derive(Deserialize)]
struct BindSignalArgs {
  signo: i32,
}

#[cfg(unix)]
#[derive(Deserialize)]
struct SignalArgs {
  rid: i32,
}

#[cfg(unix)]
fn op_signal_bind(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  state.check_unstable("Deno.signal");
  let args: BindSignalArgs = serde_json::from_value(args)?;
  let rid = state.resource_table.borrow_mut().add(
    "signal",
    Box::new(SignalStreamResource(
      signal(SignalKind::from_raw(args.signo)).expect(""),
      None,
    )),
  );
  Ok(json!({
    "rid": rid,
  }))
}

#[cfg(unix)]
async fn op_signal_poll(
  state: Rc<State>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, ErrBox> {
  state.check_unstable("Deno.signal");
  let args: SignalArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;

  let future = poll_fn(move |cx| {
    let mut resource_table = state.resource_table.borrow_mut();
    if let Some(mut signal) =
      resource_table.get_mut::<SignalStreamResource>(rid)
    {
      signal.1 = Some(cx.waker().clone());
      return signal.0.poll_recv(cx);
    }
    std::task::Poll::Ready(None)
  });
  let result = future.await;
  Ok(json!({ "done": result.is_none() }))
}

#[cfg(unix)]
pub fn op_signal_unbind(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  state.check_unstable("Deno.signal");
  let mut resource_table = state.resource_table.borrow_mut();
  let args: SignalArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let resource = resource_table.get_mut::<SignalStreamResource>(rid);
  if let Some(signal) = resource {
    if let Some(waker) = &signal.1 {
      // Wakes up the pending poll if exists.
      // This prevents the poll future from getting stuck forever.
      waker.clone().wake();
    }
  }
  resource_table
    .close(rid)
    .ok_or_else(ErrBox::bad_resource_id)?;
  Ok(json!({}))
}

#[cfg(not(unix))]
pub fn op_signal_bind(
  _state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  unimplemented!();
}

#[cfg(not(unix))]
fn op_signal_unbind(
  _state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  unimplemented!();
}

#[cfg(not(unix))]
async fn op_signal_poll(
  _state: Rc<State>,
  _args: Value,
  _zero_copy: BufVec,
) -> Result<Value, ErrBox> {
  unimplemented!();
}
