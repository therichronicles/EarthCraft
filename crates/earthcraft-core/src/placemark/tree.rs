//! Editable place hierarchy. IDs are session-local and survive moves and snapshots.
use super::Placemark;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub enum PlaceItem {
    Folder {
        name: String,
        description: String,
        document: bool,
        children: Vec<PlaceNode>,
    },
    Marker(Placemark),
}

#[derive(Clone, Debug)]
pub struct PlaceNode {
    id: u64,
    pub visible: bool,
    pub item: PlaceItem,
}
impl PlaceNode {
    pub fn marker(marker: Placemark) -> Self {
        Self::new(PlaceItem::Marker(marker))
    }
    pub fn folder(name: String, description: String, document: bool, children: Vec<Self>) -> Self {
        Self::new(PlaceItem::Folder {
            name,
            description,
            document,
            children,
        })
    }
    fn new(item: PlaceItem) -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            visible: true,
            item,
        }
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn name(&self) -> &str {
        match &self.item {
            PlaceItem::Folder { name, .. } => name,
            PlaceItem::Marker(pm) => &pm.name,
        }
    }
    pub fn name_mut(&mut self) -> &mut String {
        match &mut self.item {
            PlaceItem::Folder { name, .. } => name,
            PlaceItem::Marker(pm) => &mut pm.name,
        }
    }
    pub fn description_mut(&mut self) -> &mut String {
        match &mut self.item {
            PlaceItem::Folder { description, .. } => description,
            PlaceItem::Marker(pm) => &mut pm.desc,
        }
    }
    fn contains(&self, id: u64) -> bool {
        self.id == id
            || match &self.item {
                PlaceItem::Folder { children, .. } => children.iter().any(|node| node.contains(id)),
                PlaceItem::Marker(_) => false,
            }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Places {
    pub roots: Vec<PlaceNode>,
}
impl Places {
    pub fn push(&mut self, marker: Placemark) {
        self.roots.push(PlaceNode::marker(marker));
    }
    pub fn extend(&mut self, other: Self) {
        self.roots.extend(other.roots);
    }
    pub fn len(&self) -> usize {
        self.markers(false).count()
    }
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }
    pub fn first(&self) -> Option<&Placemark> {
        self.markers(false).next()
    }
    pub fn markers(&self, visible_only: bool) -> impl Iterator<Item = &Placemark> {
        let mut stack = vec![self.roots.iter()];
        std::iter::from_fn(move || {
            loop {
                let iter = stack.last_mut()?;
                match iter.next() {
                    None => {
                        stack.pop();
                    }
                    Some(node) if visible_only && !node.visible => {}
                    Some(node) => match &node.item {
                        PlaceItem::Marker(marker) => return Some(marker),
                        PlaceItem::Folder { children, .. } => stack.push(children.iter()),
                    },
                }
            }
        })
    }
    pub fn find_mut(&mut self, id: u64) -> Option<&mut PlaceNode> {
        fn find(nodes: &mut [PlaceNode], id: u64) -> Option<&mut PlaceNode> {
            for node in nodes {
                if node.id == id {
                    return Some(node);
                }
                if let PlaceItem::Folder { children, .. } = &mut node.item
                    && let Some(found) = find(children, id)
                {
                    return Some(found);
                }
            }
            None
        }
        find(&mut self.roots, id)
    }
    pub fn remove(&mut self, id: u64) -> Option<PlaceNode> {
        fn remove(nodes: &mut Vec<PlaceNode>, id: u64) -> Option<PlaceNode> {
            if let Some(index) = nodes.iter().position(|node| node.id == id) {
                return Some(nodes.remove(index));
            }
            for node in nodes {
                if let PlaceItem::Folder { children, .. } = &mut node.item
                    && let Some(found) = remove(children, id)
                {
                    return Some(found);
                }
            }
            None
        }
        remove(&mut self.roots, id)
    }
    pub fn insert(&mut self, parent: Option<u64>, node: PlaceNode) {
        if let Some(parent) = parent.and_then(|id| self.find_mut(id))
            && let PlaceItem::Folder { children, .. } = &mut parent.item
        {
            children.push(node);
            return;
        }
        self.roots.push(node);
    }
    /// Move to the end of a folder/root. Reject cycles and missing/non-folder targets.
    pub fn move_into(&mut self, id: u64, parent: Option<u64>) -> bool {
        let Some(node) = self.find_mut(id) else {
            return false;
        };
        if parent.is_some_and(|parent| node.contains(parent)) {
            return false;
        }
        if let Some(parent) = parent
            && !self
                .find_mut(parent)
                .is_some_and(|node| matches!(node.item, PlaceItem::Folder { .. }))
        {
            return false;
        }
        let Some(node) = self.remove(id) else {
            return false;
        };
        self.insert(parent, node);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visibility_moves_and_ids_preserve_hierarchy() {
        let mark = PlaceNode::marker(Placemark {
            name: "Peak".into(),
            desc: String::new(),
            lat: 27.0,
            lon: 86.0,
        });
        let mark_id = mark.id();
        let nested = PlaceNode::folder("Asia".into(), String::new(), false, vec![mark]);
        let nested_id = nested.id();
        let root = PlaceNode::folder("My Places".into(), String::new(), true, vec![nested]);
        let root_id = root.id();
        let mut places = Places { roots: vec![root] };
        assert!(!places.move_into(root_id, Some(nested_id)));
        assert!(!places.move_into(nested_id, Some(mark_id)));
        places.find_mut(nested_id).unwrap().visible = false;
        assert_eq!(places.markers(true).count(), 0);
        assert_eq!(places.len(), 1);
        assert!(places.move_into(mark_id, None));
        assert_eq!(places.markers(true).next().unwrap().name, "Peak");
        assert_eq!(places.roots[1].id(), mark_id);
        assert_eq!(places.clone().roots[0].id(), root_id);
        assert!(places.remove(nested_id).is_some());
        assert!(places.find_mut(mark_id).is_some());
    }
}
