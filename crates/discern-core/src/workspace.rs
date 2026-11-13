use crate::Document;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(u64);

/// Open documents (tabs). Connections and result sets will live here too.
#[derive(Debug, Default)]
pub struct Workspace {
    next_id: u64,
    documents: HashMap<DocumentId, Document>,
    pub active: Option<DocumentId>,
}

impl Workspace {
    pub fn open(&mut self, doc: Document) -> DocumentId {
        let id = DocumentId(self.next_id);
        self.next_id += 1;
        self.documents.insert(id, doc);
        self.active = Some(id);
        id
    }

    pub fn get(&self, id: DocumentId) -> Option<&Document> { self.documents.get(&id) }
    pub fn get_mut(&mut self, id: DocumentId) -> Option<&mut Document> { self.documents.get_mut(&id) }
}
