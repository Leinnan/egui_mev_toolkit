use std::{
    borrow::Cow,
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
    ops::RangeInclusive,
};

use bevy_reflect::{Reflect, attributes::CustomAttributes};

use crate::{Inspector, attribute_provider::AttributeProvider, label_input};

pub trait Num: egui::emath::Numeric {
    fn min_value() -> Self;
    fn max_value() -> Self;
}

macro_rules! impl_num {
    ($($ty:ty),*) => {
        $(
            impl Num for $ty {
                fn min_value() -> Self {
                    <$ty>::MIN
                }
                fn max_value() -> Self {
                    <$ty>::MAX
                }
            }
        )*
    };
}
impl_num!(
    f32, f64, i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, NonZeroU16, NonZeroU32,
    NonZeroU64
);
#[derive(Clone)]
#[non_exhaustive]
pub struct NumberOptions<T> {
    pub min: Option<T>,
    pub max: Option<T>,
    pub speed: f32,
}
impl<T: Num> Default for NumberOptions<T> {
    fn default() -> Self {
        NumberOptions {
            min: None,
            max: None,
            speed: 0.1,
        }
    }
}

impl<T: Num> NumberOptions<T> {
    pub fn range(&self) -> RangeInclusive<T> {
        match (self.min, self.max) {
            (Some(min), Some(max)) => min..=max,
            (Some(min), None) => min..=T::max_value(),
            (None, Some(max)) => T::min_value()..=max,
            (None, None) => T::min_value()..=T::max_value(),
        }
    }
}

impl<T: Num + Reflect> Inspector for T {
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        options: Option<&CustomAttributes>,
        id: egui::Id,
        _reg: &bevy_reflect::TypeRegistry,
    ) -> bool {
        let options = match options.and_then(|o| o.ext_copy_attribute::<RangeInclusive<T>>()) {
            Some(s) => NumberOptions {
                min: (*s.start()).into(),
                max: (*s.end()).into(),
                speed: 0.1,
            },
            None => NumberOptions::<T>::default(),
        };
        label_input(
            ui.add(
                egui::DragValue::new(self)
                    .range(options.range())
                    .speed(options.speed),
            ),
            id,
        )
        .changed()
    }
}

impl Inspector for String {
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _: Option<&CustomAttributes>,
        id: egui::Id,
        _: &bevy_reflect::TypeRegistry,
    ) -> bool {
        label_input(ui.text_edit_singleline(self), id).changed()
    }
}

impl Inspector for Cow<'static, str> {
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _: Option<&CustomAttributes>,
        id: egui::Id,
        _: &bevy_reflect::TypeRegistry,
    ) -> bool {
        let mut clone = self.to_string();
        let response = if self.contains('\n') {
            ui.text_edit_multiline(&mut clone)
        } else {
            ui.text_edit_singleline(&mut clone)
        };
        let changed = label_input(response, id).changed();

        if changed {
            *self = Cow::Owned(clone);
        }

        changed
    }
}

impl Inspector for bool {
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _: Option<&CustomAttributes>,
        id: egui::Id,
        _: &bevy_reflect::TypeRegistry,
    ) -> bool {
        label_input(ui.checkbox(self, ()), id).changed()
    }
}
impl Inspector for core::time::Duration {
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _: Option<&CustomAttributes>,
        id: egui::Id,
        _: &bevy_reflect::TypeRegistry,
    ) -> bool {
        let mut time = self.as_secs_f64();
        let response = ui.add(
            egui::DragValue::new(&mut time)
                .speed(0.01)
                .suffix("s")
                .range(0.0..=f64::MAX),
        );
        if label_input(response, id).changed() {
            *self = core::time::Duration::from_secs_f64(time);
            true
        } else {
            false
        }
    }
}

pub fn is_option_type(enum_info: &bevy_reflect::enums::EnumInfo) -> bool {
    enum_info.variant_len() == 2
        && enum_info.generics().len() == 1
        && enum_info.contains_variant("None")
        && enum_info.contains_variant("Some")
}

pub fn all_unit_variants(enum_info: &bevy_reflect::enums::EnumInfo) -> bool {
    enum_info.iter().all(|variant| {
        variant
            .variant_type()
            .eq(&bevy_reflect::enums::VariantType::Unit)
    })
}
