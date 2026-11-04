use bevy_reflect::{NamedField, attributes::CustomAttributes};

pub trait AttributeProvider: std::any::Any {
    fn ext_get_attribute_by_id(
        &self,
        id: ::core::any::TypeId,
    ) -> Option<&dyn bevy_reflect::Reflect>;
    fn ext_copy_attribute<T: Clone + 'static>(&self) -> Option<T> {
        self.ext_get_attribute_by_id(::core::any::TypeId::of::<T>())
            .and_then(|attr| attr.downcast_ref::<T>().cloned())
    }
}
impl AttributeProvider for CustomAttributes {
    fn ext_get_attribute_by_id(
        &self,
        id: ::core::any::TypeId,
    ) -> Option<&dyn bevy_reflect::Reflect> {
        self.get_by_id(id)
    }
}
impl AttributeProvider for NamedField {
    fn ext_get_attribute_by_id(
        &self,
        id: ::core::any::TypeId,
    ) -> Option<&dyn bevy_reflect::Reflect> {
        self.get_attribute_by_id(id)
    }
}
