use std::rc::Rc;

use crate::{object::Obj, value::Value};

const TABLE_MAX_LOAD: f32 = 0.75;

struct Entry {
    key: Rc<Obj>,
    value: Value,
}
pub(crate) struct Table {
    count: usize,
    entries: Vec<Option<Entry>>,
}

impl Table {
    pub(crate) fn new() -> Table {
        let mut entries = Vec::with_capacity(8);
        entries.fill_with(|| None);
        Table { count: 0, entries }
    }

    pub(crate) fn set(&mut self, key: Rc<Obj>, value: Value) -> bool {
        if self.count + 1 > (self.entries.capacity() as f32 * TABLE_MAX_LOAD).floor() as usize {
            let mut new_entries = Vec::with_capacity(self.entries.capacity() * 2);
            new_entries.fill_with(|| None);
            for entry in &self.entries {
                match entry {
                    Some(entry) => {
                        let index = Table::find_entry(&new_entries, &entry.key);
                        Table::place_entry(&mut new_entries, index, key.clone(), value.clone());
                    }
                    None => continue,
                }
            }
            self.entries = new_entries;
        }

        let index = Table::find_entry(&self.entries, &key);
        let is_new_value = Table::place_entry(&mut self.entries, index, key, value);
        if is_new_value {
            self.count += 1;
        }
        is_new_value
    }

    fn get(&mut self, key: &Rc<Obj>) -> Option<&Value> {
        if self.count == 0 {
            return None;
        };

        let entry = Table::find_entry(&self.entries, key);
        match entry {
            Ok(index) => Some(&self.entries[index].as_ref().unwrap().value),
            Err(_) => None,
        }
    }

    fn place_entry(
        entries: &mut [Option<Entry>],
        index: Result<usize, usize>,
        key: Rc<Obj>,
        value: Value,
    ) -> bool {
        match index {
            Ok(index) => {
                entries[index] = Some(Entry { key, value });
                false
            }
            Err(index) => {
                entries[index] = Some(Entry { key, value });
                true
            }
        }
    }

    fn find_entry(entries: &[Option<Entry>], key: &Rc<Obj>) -> Result<usize, usize> {
        let Obj::String { hash, .. } = **key else {
            panic!("Table key must be a string.")
        };

        let mut index = hash as usize % entries.len();
        loop {
            match entries.get(index) {
                Some(Some(entry)) if entry.key == *key => return Ok(index),
                None => return Err(index),
                _ => index = (index + 1) % entries.len(),
            }
        }
    }

    fn add_all(from: &mut Table, to: &mut Table) {
        for entry in from.entries.iter().flatten() {
            to.set(entry.key.clone(), entry.value.clone());
        }
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}
