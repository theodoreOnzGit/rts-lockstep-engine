// SPDX-License-Identifier: GPL-3.0-only
//! The one client-notification interface: a per-frame queue of
//! [`ClientEvent`]s that logic owns and the client drains.
//!
//! Logic never calls the client. Everything upstream sends to `TheAudio`,
//! `TheInGameUI`, `TheControlBar`, `TheEva`, `TheRadar`,
//! `TheParticleSystemManager`, `TheTerrainVisual`, `TheTacticalView`,
//! `TheGameClient` and `TheGhostObjectManager` is a [`ClientEvent`] pushed onto
//! this queue (the cut-point section of `docs/phase1-porting-graph.md`;
//! `docs/world-design.md` has the rule for the reads that go the other way).
//!
//! - **Order is the order of the calls**, which is the order of upstream's
//!   synchronous calls. The client applies the events in that order.
//! - **The queue is not part of the simulation state**: it is never read by
//!   logic, never saved, and not in the CRC. A headless run may [`ClientEventQueue::drain`]
//!   and drop it every frame, or keep it for a test to compare.
//! - **Unbounded growth is the caller's problem**: a headless run that never
//!   drains grows the queue without limit; call [`ClientEventQueue::clear`]
//!   once a frame.

use crate::logic::client_event::ClientEvent;

/// The queue. See the module docs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientEventQueue {
    events: Vec<ClientEvent>,
}

impl ClientEventQueue {
    /// An empty queue.
    pub fn new() -> ClientEventQueue {
        ClientEventQueue::default()
    }

    /// Appends one event.
    pub fn push(&mut self, event: ClientEvent) {
        self.events.push(event);
    }

    /// Appends all of `events`, in order.
    pub fn extend<I: IntoIterator<Item = ClientEvent>>(&mut self, events: I) {
        self.events.extend(events);
    }

    /// The backing vector, for the translated code whose signature already
    /// takes `&mut Vec<ClientEvent>` (`Money::deposit`, `ThingFactory::new_drawable`).
    pub fn as_vec_mut(&mut self) -> &mut Vec<ClientEvent> {
        &mut self.events
    }

    /// The events not yet drained, oldest first.
    pub fn events(&self) -> &[ClientEvent] {
        &self.events
    }

    /// The number of events waiting.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Nothing waiting.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// The client's call after each logic frame: every waiting event, oldest
    /// first, leaving the queue empty.
    pub fn drain(&mut self) -> Vec<ClientEvent> {
        std::mem::take(&mut self.events)
    }

    /// Drops every waiting event (the headless run that does not look).
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::client_event::MoneySound;

    #[test]
    fn events_drain_in_order_and_leave_the_queue_empty() {
        let mut q = ClientEventQueue::new();
        assert!(q.is_empty());
        q.push(ClientEvent::MoneySound {
            sound: MoneySound::Deposit,
            player_index: 1,
        });
        q.extend([
            ClientEvent::ObjectDestroyed { object_id: 4 },
            ClientEvent::ObjectDestroyed { object_id: 5 },
        ]);
        q.as_vec_mut()
            .push(ClientEvent::ObjectDestroyed { object_id: 6 });
        assert_eq!(q.len(), 4);
        let got = q.drain();
        assert_eq!(got.len(), 4);
        assert_eq!(got[1], ClientEvent::ObjectDestroyed { object_id: 4 });
        assert!(q.is_empty());
        q.push(ClientEvent::ObjectDestroyed { object_id: 1 });
        q.clear();
        assert!(q.drain().is_empty());
    }
}
