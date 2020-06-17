use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::rc::Rc;

pub struct TypeMap {
    map: HashMap<TypeId, Rc<dyn Any>>,
}

impl TypeMap {
    pub fn new() -> TypeMap {
        TypeMap {
            map: HashMap::new(),
        }
    }

    pub fn insert<T: Any>(&mut self, value: T) {
        self.map.insert(TypeId::of::<T>(), Rc::new(value));
    }

    pub fn insert_any(&mut self, value: Box<dyn Any>) {
        let type_id = Any::type_id(value.as_ref());
        self.map.insert(type_id, value.into());
    }

    pub fn get<T: Any>(&self) -> Option<Rc<T>> {
        self.map
            .get(&TypeId::of::<T>())
            .map(Rc::clone)
            .map(Rc::downcast::<T>)
            .and_then(Result::ok)
    }

    pub fn get_ref<T: Any>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .map(Rc::as_ref)
            .and_then(Any::downcast_ref::<T>)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let mut map = TypeMap::new();

        map.insert(123i32);
        map.insert(456u32);

        assert_eq!(Some(&123i32), map.get_ref::<i32>());
        assert_eq!(Some(&456u32), map.get_ref::<u32>());
        assert_eq!(None, map.get_ref::<usize>());

        map.insert_any(Box::new(100i32));

        assert_eq!(Some(&100i32), map.get_ref::<i32>());
        assert_eq!(Some(&456u32), map.get_ref::<u32>());
        assert_eq!(None, map.get_ref::<usize>());
    }
}
