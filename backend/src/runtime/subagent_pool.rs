//! Sub-Agent Pool Management
//!
//! Manages a pool of slots for sub-agent execution, supporting
//! allocation, release, and status tracking.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Status of a pool slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlotStatus {
    /// Slot is available for allocation.
    Idle,
    /// Slot is currently in use.
    Busy,
    /// Slot has been detached from the pool.
    Detached,
}

/// A single slot in the sub-agent pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotAllocation {
    /// Unique slot identifier.
    pub slot_id: usize,
    /// Workflow ID using this slot, if any.
    pub workflow_id: Option<String>,
    /// Current status of the slot.
    pub status: SlotStatus,
}

impl SlotAllocation {
    /// Create a new idle slot.
    pub fn new(slot_id: usize) -> Self {
        Self {
            slot_id,
            workflow_id: None,
            status: SlotStatus::Idle,
        }
    }
}

/// Status information for the entire pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStatus {
    /// Total number of slots in the pool.
    pub total_slots: usize,
    /// Number of idle slots.
    pub idle_slots: usize,
    /// Number of busy slots.
    pub busy_slots: usize,
    /// Number of detached slots.
    pub detached_slots: usize,
    /// Details of each slot.
    pub slots: Vec<SlotAllocation>,
}

/// A pool of sub-agent execution slots.
pub struct SubagentPool {
    slots: Vec<Arc<Mutex<SlotAllocation>>>,
    max_slots: usize,
}

impl SubagentPool {
    /// Create a new sub-agent pool with the specified maximum slots.
    pub fn new(max_slots: usize) -> Self {
        let slots = (0..max_slots)
            .map(|i| Arc::new(Mutex::new(SlotAllocation::new(i))))
            .collect();

        Self { slots, max_slots }
    }

    /// Allocate a number of slots, returning their IDs.
    ///
    /// Returns as many slot IDs as available, up to the requested count.
    pub async fn allocate(&self, count: usize) -> Vec<usize> {
        let mut allocated = Vec::with_capacity(count);

        for slot in &self.slots {
            if allocated.len() >= count {
                break;
            }

            let mut slot_guard = slot.lock().await;
            if slot_guard.status == SlotStatus::Idle {
                slot_guard.status = SlotStatus::Busy;
                allocated.push(slot_guard.slot_id);
            }
        }

        allocated
    }

    /// Release previously allocated slots back to the pool.
    pub async fn release(&self, slot_ids: &[usize]) {
        let slot_ids_set: std::collections::HashSet<usize> =
            slot_ids.iter().copied().collect();

        for slot in &self.slots {
            let mut slot_guard = slot.lock().await;
            if slot_ids_set.contains(&slot_guard.slot_id)
                && slot_guard.status == SlotStatus::Busy
            {
                slot_guard.status = SlotStatus::Idle;
                slot_guard.workflow_id = None;
            }
        }
    }

    /// Detach a slot from the pool (permanent removal).
    pub async fn detach(&self, slot_id: usize) -> bool {
        for slot in &self.slots {
            let mut slot_guard = slot.lock().await;
            if slot_guard.slot_id == slot_id && slot_guard.status != SlotStatus::Detached {
                slot_guard.status = SlotStatus::Detached;
                slot_guard.workflow_id = None;
                return true;
            }
        }
        false
    }

    /// Get the current status of the pool.
    pub async fn status(&self) -> PoolStatus {
        let mut idle = 0;
        let mut busy = 0;
        let mut detached = 0;
        let mut slots_info = Vec::with_capacity(self.max_slots);

        for slot in &self.slots {
            let slot_guard = slot.lock().await;
            match slot_guard.status {
                SlotStatus::Idle => idle += 1,
                SlotStatus::Busy => busy += 1,
                SlotStatus::Detached => detached += 1,
            }
            slots_info.push(slot_guard.clone());
        }

        PoolStatus {
            total_slots: self.max_slots,
            idle_slots: idle,
            busy_slots: busy,
            detached_slots: detached,
            slots: slots_info,
        }
    }

    /// Get a specific slot's allocation info.
    pub async fn get_slot(&self, slot_id: usize) -> Option<SlotAllocation> {
        for slot in &self.slots {
            let slot_guard = slot.lock().await;
            if slot_guard.slot_id == slot_id {
                return Some(slot_guard.clone());
            }
        }
        None
    }

    /// Update the workflow ID for a busy slot.
    pub async fn set_workflow_id(&self, slot_id: usize, workflow_id: Option<String>) {
        for slot in &self.slots {
            let mut slot_guard = slot.lock().await;
            if slot_guard.slot_id == slot_id && slot_guard.status == SlotStatus::Busy {
                slot_guard.workflow_id = workflow_id;
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_allocate_and_release() {
        let pool = SubagentPool::new(4);

        // Initially all slots should be idle
        let status = pool.status().await;
        assert_eq!(status.idle_slots, 4);
        assert_eq!(status.busy_slots, 0);

        // Allocate 2 slots
        let allocated = pool.allocate(2).await;
        assert_eq!(allocated.len(), 2);

        let status = pool.status().await;
        assert_eq!(status.idle_slots, 2);
        assert_eq!(status.busy_slots, 2);

        // Release the slots
        pool.release(&allocated).await;

        let status = pool.status().await;
        assert_eq!(status.idle_slots, 4);
        assert_eq!(status.busy_slots, 0);
    }

    #[tokio::test]
    async fn test_allocate_more_than_available() {
        let pool = SubagentPool::new(2);

        // Allocate more than available
        let allocated = pool.allocate(5).await;
        assert_eq!(allocated.len(), 2); // Should only get 2

        let status = pool.status().await;
        assert_eq!(status.idle_slots, 0);
        assert_eq!(status.busy_slots, 2);
    }

    #[tokio::test]
    async fn test_detach_slot() {
        let pool = SubagentPool::new(2);

        let allocated = pool.allocate(1).await;
        assert_eq!(allocated.len(), 1);

        let detached = pool.detach(allocated[0]).await;
        assert!(detached);

        let status = pool.status().await;
        assert_eq!(status.detached_slots, 1);
        assert_eq!(status.busy_slots, 0); // Detached slots are not counted as busy
    }

    #[tokio::test]
    async fn test_set_workflow_id() {
        let pool = SubagentPool::new(1);

        let allocated = pool.allocate(1).await;
        pool.set_workflow_id(allocated[0], Some("wf-123".to_string()))
            .await;

        let slot = pool.get_slot(allocated[0]).await;
        assert!(slot.is_some());
        assert_eq!(slot.unwrap().workflow_id, Some("wf-123".to_string()));
    }

    #[test]
    fn test_slot_allocation_new() {
        let slot = SlotAllocation::new(0);
        assert_eq!(slot.slot_id, 0);
        assert!(slot.workflow_id.is_none());
        assert_eq!(slot.status, SlotStatus::Idle);
    }
}
