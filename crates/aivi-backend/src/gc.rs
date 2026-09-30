use std::marker::PhantomData;

use crate::RuntimeValue;

/// Storage contract for committed scheduler values.
///
/// Pending evaluator results stay ordinary Rust-owned values until the scheduler commits them at a
/// tick boundary. Stores own only those committed snapshots, which lets the runtime introduce
/// relocation behind stable handles without widening worker/GTK/source boundary contracts.
pub trait CommittedValueStore<V> {
    type Slot: Default;

    fn get<'a>(&'a self, slot: &'a Self::Slot) -> Option<&'a V>;

    fn replace(&mut self, slot: &mut Self::Slot, value: V);

    fn clear(&mut self, slot: &mut Self::Slot) -> bool;

    /// Whether a scheduler safe point needs a root scan. Explicit `collect` still forces it.
    fn needs_collection(&self) -> bool {
        true
    }

    fn collect(&mut self, roots: &[&Self::Slot]);
}

/// Inline committed-value storage used by non-GC scheduler instantiations.
pub struct InlineCommittedValueStore<V> {
    marker: PhantomData<fn() -> V>,
}

impl<V> Default for InlineCommittedValueStore<V> {
    fn default() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

impl<V> CommittedValueStore<V> for InlineCommittedValueStore<V> {
    type Slot = Option<V>;

    fn get<'a>(&'a self, slot: &'a Self::Slot) -> Option<&'a V> {
        slot.as_ref()
    }

    fn replace(&mut self, slot: &mut Self::Slot, value: V) {
        *slot = Some(value);
    }

    fn clear(&mut self, slot: &mut Self::Slot) -> bool {
        slot.take().is_some()
    }

    fn collect(&mut self, _roots: &[&Self::Slot]) {}

    fn needs_collection(&self) -> bool {
        false
    }
}

/// Stable root handle for scheduler-owned runtime values.
///
/// The `(slot, generation)` pair never exposes an object address directly. Collections may freely
/// relocate values between spaces while scheduler slots retain the same live handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuntimeGcHandle {
    slot: u32,
    generation: u32,
}

impl RuntimeGcHandle {
    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

/// Moving store for committed `RuntimeValue` snapshots.
///
/// This initial moving slice is intentionally narrow: the store owns only committed scheduler
/// values. Each live scheduler slot holds one stable root handle. Collections move owned values
/// into a fresh space and rewrite root slots. Nested buffers transfer ownership without copying;
/// their addresses are not part of the public contract. Evaluator temporaries remain outside
/// this store, and no value can contain a collector object ID.
///
/// # Thread safety
///
/// `MovingRuntimeValueStore` must only be accessed from a single thread at a time.
/// All methods MUST be called from the owning thread. No internal synchronization
/// is provided.
#[derive(Default)]
pub struct MovingRuntimeValueStore {
    from_space: RuntimeGcSpace,
    to_space: RuntimeGcSpace,
    roots: Vec<RuntimeGcRootSlot>,
    free_roots: Vec<u32>,
    root_worklist: Vec<(RuntimeGcHandle, RuntimeGcObjectId)>,
    collections: u64,
    live_roots: usize,
}

impl MovingRuntimeValueStore {
    pub fn collection_count(&self) -> u64 {
        self.collections
    }

    pub fn live_root_count(&self) -> usize {
        self.live_roots
    }

    pub fn allocated_value_count(&self) -> usize {
        self.from_space.values.len()
    }

    fn allocate_root(&mut self, value: RuntimeValue) -> RuntimeGcHandle {
        let object = self.from_space.push(value);
        self.live_roots += 1;
        if let Some(slot_index) = self.free_roots.pop() {
            let slot = &mut self.roots[slot_index as usize];
            debug_assert!(
                slot.object.is_none(),
                "free root slots must not keep live object ids"
            );
            slot.object = Some(object);
            RuntimeGcHandle {
                slot: slot_index,
                generation: slot.generation,
            }
        } else {
            let slot_index = self.roots.len() as u32;
            self.roots.push(RuntimeGcRootSlot {
                generation: 0,
                object: Some(object),
            });
            RuntimeGcHandle {
                slot: slot_index,
                generation: 0,
            }
        }
    }

    fn root_slot(&self, handle: RuntimeGcHandle) -> &RuntimeGcRootSlot {
        let slot = self
            .roots
            .get(handle.slot as usize)
            .expect("moving-GC root handles must reference an allocated slot");
        assert_eq!(
            slot.generation, handle.generation,
            "moving-GC root handles must not outlive their generation"
        );
        slot
    }

    // SAFETY: There is no synchronization on this accessor. `MovingRuntimeValueStore` must only
    // be accessed from the owning thread. The caller is responsible for ensuring external
    // synchronization if the store is shared across threads.
    // Calling this from multiple threads without external synchronization is undefined behavior
    // and would allow data races on the root-slot vector.
    fn root_slot_mut(&mut self, handle: RuntimeGcHandle) -> &mut RuntimeGcRootSlot {
        let slot = self
            .roots
            .get_mut(handle.slot as usize)
            .expect("moving-GC root handles must reference an allocated slot");
        assert_eq!(
            slot.generation, handle.generation,
            "moving-GC root handles must not outlive their generation"
        );
        slot
    }

    fn resolve_handle(&self, handle: RuntimeGcHandle) -> &RuntimeValue {
        let object = self
            .root_slot(handle)
            .object
            .expect("moving-GC root handles must always point at a live object");
        self.from_space.get(object)
    }

    fn recycle_root(&mut self, handle: RuntimeGcHandle) -> bool {
        let slot = self.root_slot_mut(handle);
        let had_object = slot.object.take().is_some();
        if had_object {
            slot.generation = slot.generation.wrapping_add(1);
            self.live_roots = self
                .live_roots
                .checked_sub(1)
                .expect("clearing a live root must not underflow the root count");
            self.free_roots.push(handle.slot);
        }
        had_object
    }
}

impl CommittedValueStore<RuntimeValue> for MovingRuntimeValueStore {
    type Slot = Option<RuntimeGcHandle>;

    fn get<'a>(&'a self, slot: &'a Self::Slot) -> Option<&'a RuntimeValue> {
        slot.as_ref().map(|handle| self.resolve_handle(*handle))
    }

    fn replace(&mut self, slot: &mut Self::Slot, value: RuntimeValue) {
        if let Some(handle) = slot.as_ref().copied() {
            let object = self
                .root_slot(handle)
                .object
                .expect("live moving-GC roots must point at an allocated object");
            self.from_space.replace(object, value);
            return;
        }

        *slot = Some(self.allocate_root(value));
    }

    fn clear(&mut self, slot: &mut Self::Slot) -> bool {
        let Some(handle) = slot.take() else {
            return false;
        };
        self.recycle_root(handle)
    }

    fn collect(&mut self, roots: &[&Self::Slot]) {
        self.root_worklist.clear();
        // Detach object IDs before relocating anything. Repeated roots are harmless:
        // only the first occurrence takes the ID. Exclusive access hides this transient state.
        for handle in roots.iter().filter_map(|slot| slot.as_ref().copied()) {
            if let Some(object) = self.root_slot_mut(handle).object.take() {
                self.root_worklist.push((handle, object));
            }
        }
        self.to_space.clear();
        self.to_space.values.reserve(self.root_worklist.len());
        for index in 0..self.root_worklist.len() {
            let (handle, object) = self.root_worklist[index];
            let value = std::mem::replace(
                &mut self.from_space.values[object.0 as usize],
                RuntimeValue::Unit,
            );
            let relocated = self.to_space.push(value);
            self.root_slot_mut(handle).object = Some(relocated);
        }

        std::mem::swap(&mut self.from_space, &mut self.to_space);
        self.to_space.clear();
        self.collections = self.collections.wrapping_add(1);
    }

    fn needs_collection(&self) -> bool {
        self.from_space.values.len() > self.live_roots
    }
}

#[derive(Default)]
struct RuntimeGcSpace {
    values: Vec<RuntimeValue>,
}

impl RuntimeGcSpace {
    fn clear(&mut self) {
        for value in &mut self.values {
            value.discard_tree_in_place();
        }
        self.values.clear();
    }

    fn push(&mut self, value: RuntimeValue) -> RuntimeGcObjectId {
        let index = self.values.len() as u32;
        self.values.push(value);
        RuntimeGcObjectId(index)
    }

    fn get(&self, id: RuntimeGcObjectId) -> &RuntimeValue {
        self.values
            .get(id.0 as usize)
            .expect("moving-GC object ids must reference the active space")
    }

    /// Replaces one opaque root payload at the collector's only object-mutation boundary.
    ///
    /// No write barrier is required by the current stop-the-world copying collector:
    /// `RuntimeValue` cannot contain a [`RuntimeGcObjectId`], so replacement cannot introduce an
    /// edge between GC-managed objects. If runtime values later become a graph of managed
    /// objects, this method is the mandatory barrier insertion point and must record the old/new
    /// generation or incremental-marking relationship before performing the write.
    fn replace(&mut self, id: RuntimeGcObjectId, value: RuntimeValue) {
        let object = self
            .values
            .get_mut(id.0 as usize)
            .expect("moving-GC object ids must reference the active space");
        object.discard_tree_in_place();
        *object = value;
    }
}

impl Drop for RuntimeGcSpace {
    fn drop(&mut self) {
        self.clear();
    }
}

#[derive(Clone, Copy)]
struct RuntimeGcObjectId(u32);

struct RuntimeGcRootSlot {
    generation: u32,
    object: Option<RuntimeGcObjectId>,
}

#[cfg(test)]
mod tests {
    use super::{CommittedValueStore, MovingRuntimeValueStore, RuntimeGcHandle, RuntimeValue};

    fn text_ptr(value: &RuntimeValue) -> *const u8 {
        let RuntimeValue::Text(text) = value else {
            panic!("expected text runtime value");
        };
        text.as_ptr()
    }

    #[test]
    fn moving_store_relocates_text_roots_without_changing_handles() {
        let mut store = MovingRuntimeValueStore::default();
        let mut slot = Option::<RuntimeGcHandle>::default();
        store.replace(&mut slot, RuntimeValue::Text("Ada".into()));
        let handle = slot.expect("store should allocate a root handle");
        let before_value = store
            .get(&slot)
            .expect("allocated root should remain readable")
            as *const RuntimeValue;
        let before_text = text_ptr(store.get(&slot).unwrap());

        let roots = [&slot];
        store.collect(&roots);

        assert_eq!(slot, Some(handle));
        assert_eq!(store.collection_count(), 1);
        assert_eq!(store.live_root_count(), 1);
        assert_eq!(store.allocated_value_count(), 1);
        let after = store
            .get(&slot)
            .expect("collected root should stay readable");
        assert_eq!(after, &RuntimeValue::Text("Ada".into()));
        assert_ne!(
            before_value, after as *const RuntimeValue,
            "moving collection must relocate the committed value object"
        );
        assert_eq!(
            before_text,
            text_ptr(after),
            "compaction must transfer nested buffer ownership without copying"
        );
    }

    #[test]
    fn compaction_preserves_lists_and_accepts_repeated_roots() {
        let mut store = MovingRuntimeValueStore::default();
        let mut slot = None;
        let values = vec![RuntimeValue::Text("kept".into()); 1024];
        let allocation = values.as_ptr();
        store.replace(&mut slot, RuntimeValue::List(values));
        assert!(!store.needs_collection());
        store.collect(&[&slot, &slot]);
        let Some(RuntimeValue::List(values)) = store.get(&slot) else {
            panic!("list root lost")
        };
        assert_eq!(values.as_ptr(), allocation);
        assert_eq!(values.len(), 1024);
        assert_eq!(values[1023], RuntimeValue::Text("kept".into()));
        assert_eq!(store.allocated_value_count(), 1);
        store.clear(&mut slot);
        assert!(store.needs_collection());
        store.collect(&[]);
        assert_eq!(store.allocated_value_count(), 0);
        assert!(!store.needs_collection());
    }

    #[test]
    fn compaction_preserves_reordered_roots_around_dead_objects() {
        let mut store = MovingRuntimeValueStore::default();
        let mut slots = [None; 4];
        for (index, slot) in slots.iter_mut().enumerate() {
            store.replace(slot, RuntimeValue::Int(index as i64));
        }
        store.clear(&mut slots[1]);
        let handles = slots;
        store.collect(&[&slots[3], &slots[0], &slots[2], &slots[0]]);
        assert_eq!(slots, handles);
        assert_eq!(store.allocated_value_count(), 3);
        for index in [0, 2, 3] {
            assert_eq!(
                store.get(&slots[index]),
                Some(&RuntimeValue::Int(index as i64))
            );
        }
        store.replace(&mut slots[1], RuntimeValue::Int(10));
        store.collect(&slots.iter().collect::<Vec<_>>());
        assert_eq!(store.get(&slots[1]), Some(&RuntimeValue::Int(10)));
        assert_eq!(store.get(&slots[3]), Some(&RuntimeValue::Int(3)));
        assert_eq!(store.allocated_value_count(), 4);
    }

    #[test]
    fn moving_store_clears_and_reuses_slots_with_new_generations() {
        let mut store = MovingRuntimeValueStore::default();
        let mut slot = Option::<RuntimeGcHandle>::default();
        store.replace(&mut slot, RuntimeValue::Text("old".into()));
        let first = slot.expect("first allocation should produce a handle");

        assert!(store.clear(&mut slot));
        assert_eq!(slot, None);
        assert_eq!(store.live_root_count(), 0);
        assert_eq!(
            store.allocated_value_count(),
            1,
            "dead objects stay in from-space until the next collection"
        );

        let no_roots: [&Option<RuntimeGcHandle>; 0] = [];
        store.collect(&no_roots);
        assert_eq!(store.allocated_value_count(), 0);

        store.replace(&mut slot, RuntimeValue::Text("new".into()));
        let second = slot.expect("re-allocation should produce a new handle");
        assert_eq!(
            first.slot(),
            second.slot(),
            "cleared root slots should be recycled instead of leaking"
        );
        assert_ne!(
            first.generation(),
            second.generation(),
            "recycled root slots must advance generation to invalidate stale handles"
        );
        assert_eq!(store.live_root_count(), 1);
    }

    #[test]
    fn moving_store_replacement_preserves_the_root_and_latest_value_across_collection() {
        let mut store = MovingRuntimeValueStore::default();
        let mut slot = Option::<RuntimeGcHandle>::default();
        store.replace(&mut slot, RuntimeValue::Int(1));
        let handle = slot.expect("first value should allocate a stable root handle");

        store.replace(&mut slot, RuntimeValue::Int(2));
        assert_eq!(slot, Some(handle));
        assert_eq!(store.allocated_value_count(), 1);
        assert_eq!(store.get(&slot), Some(&RuntimeValue::Int(2)));

        store.collect(&[&slot]);
        assert_eq!(slot, Some(handle));
        assert_eq!(store.get(&slot), Some(&RuntimeValue::Int(2)));

        store.replace(&mut slot, RuntimeValue::Int(3));
        assert_eq!(slot, Some(handle));
        assert_eq!(store.get(&slot), Some(&RuntimeValue::Int(3)));
    }
    #[test]
    fn moving_store_releases_deep_values_on_replace_collection_and_drop() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let nested = || {
                    let mut value = RuntimeValue::Int(17);
                    for _ in 0..20_000 {
                        value = RuntimeValue::Task(crate::RuntimeTaskPlan::Pure {
                            value: Box::new(value),
                        });
                    }
                    value
                };
                let mut store = MovingRuntimeValueStore::default();
                let mut slot = None;
                store.replace(&mut slot, nested());
                store.replace(&mut slot, RuntimeValue::Int(5));
                assert_eq!(store.get(&slot), Some(&RuntimeValue::Int(5)));
                store.replace(&mut slot, nested());
                store.clear(&mut slot);
                store.collect(&[]);
                assert_eq!(store.allocated_value_count(), 0);
                store.replace(&mut slot, nested());
                store.collect(&[&slot]);
                assert_eq!(store.live_root_count(), 1);
                drop(store);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
