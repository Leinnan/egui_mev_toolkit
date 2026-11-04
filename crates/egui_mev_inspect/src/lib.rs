//! A reflect-driven egui editor over `bevy_reflect`, without the `bevy` crate.
//!
//! [`show`] draws an editor for any reflected value and returns whether it
//! changed. Use a registry from [`create_registry`], and register your own types
//! on it (or enable `auto_register`) so that lists and enums can build default
//! values for them. Register [`InspectorImpl`] on a type to give it custom UI.

use bevy_reflect::{
    CreateTypeData, PartialReflect, Reflect, ReflectMut, TypeInfo, TypeRegistry,
    array::Array,
    attributes::CustomAttributes,
    enums::{DynamicEnum, DynamicVariant, Enum, VariantInfo},
    list::List,
    map::Map,
    set::Set,
    std_traits::ReflectDefault,
    structs::{DynamicStruct, Struct},
    tuple::{DynamicTuple, Tuple},
    tuple_struct::TupleStruct,
};
use egui::{Align, Layout, Margin, Response, RichText, Stroke};
use egui_mev_theme::{Metrics, Palette};
use std::{
    any::{Any, TypeId},
    borrow::Cow,
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
};

use crate::{
    button_group::{ButtonGroupElement, UiBuilderExt},
    std_impl::{all_unit_variants, is_option_type},
};

mod attribute_provider;
pub mod button_group;
#[cfg(feature = "configs")]
pub mod config;
mod std_impl;

/// A registry with the built-in editors for numbers, strings, `bool` and `Duration`.
pub fn create_registry() -> TypeRegistry {
    let mut type_registry = TypeRegistry::new();
    #[cfg(feature = "auto_register")]
    type_registry.register_derived_types();
    type_registry.register::<Cow<str>>();
    type_registry.register::<NonZeroU16>();
    type_registry.register::<NonZeroU32>();
    type_registry.register::<NonZeroU64>();
    type_registry.register::<core::time::Duration>();
    type_registry.register_type_data::<f32, InspectorImpl>();
    type_registry.register_type_data::<f64, InspectorImpl>();
    type_registry.register_type_data::<i8, InspectorImpl>();
    type_registry.register_type_data::<i16, InspectorImpl>();
    type_registry.register_type_data::<i32, InspectorImpl>();
    type_registry.register_type_data::<i64, InspectorImpl>();
    type_registry.register_type_data::<u8, InspectorImpl>();
    type_registry.register_type_data::<u16, InspectorImpl>();
    type_registry.register_type_data::<u32, InspectorImpl>();
    type_registry.register_type_data::<u64, InspectorImpl>();
    type_registry.register_type_data::<isize, InspectorImpl>();
    type_registry.register_type_data::<usize, InspectorImpl>();
    type_registry.register_type_data::<NonZeroU16, InspectorImpl>();
    type_registry.register_type_data::<NonZeroU32, InspectorImpl>();
    type_registry.register_type_data::<NonZeroU64, InspectorImpl>();
    type_registry.register_type_data::<String, InspectorImpl>();
    type_registry.register_type_data::<Cow<str>, InspectorImpl>();
    type_registry.register_type_data::<bool, InspectorImpl>();
    type_registry.register_type_data::<core::time::Duration, InspectorImpl>();
    type_registry
}

pub trait TooltipHelper {
    fn add_tooltip(self, text: Option<&str>, on_disabled_too: bool) -> Self;
}

impl TooltipHelper for Response {
    fn add_tooltip(mut self, text: Option<&str>, on_disabled_too: bool) -> Self {
        if let Some(text) = text
            && !text.is_empty()
        {
            self = self.on_hover_text(text);
            if on_disabled_too {
                self = self.on_disabled_hover_text(text);
            }
        }
        self
    }
}

/// Custom UI for a type. Register [`InspectorImpl`] on it to use it.
pub trait Inspector: Reflect {
    /// Draws the value and returns whether it changed. Pass input widgets through
    /// [`label_input`] with `id` to name them for screen readers.
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        options: Option<&CustomAttributes>,
        id: egui::Id,
        registry: &TypeRegistry,
    ) -> bool;
}

pub trait InspectorName: Reflect {
    fn name_ui(
        &self,
        ui: &mut egui::Ui,
        options: Option<&CustomAttributes>,
        id: egui::Id,
        registry: &TypeRegistry,
    ) -> Response;
}

type InspectorImplFn =
    fn(&mut dyn Any, &mut egui::Ui, Option<&CustomAttributes>, egui::Id, &TypeRegistry) -> bool;

type InspectorReadOnlyImplFn =
    fn(&dyn Any, &mut egui::Ui, Option<&CustomAttributes>, egui::Id, &TypeRegistry) -> Response;

#[derive(Clone)]
pub struct InspectorImpl {
    pub fn_mut: InspectorImplFn,
}

#[derive(Clone)]
pub struct CustomNameInspectorImpl {
    pub fn_ptr: InspectorReadOnlyImplFn,
}

impl<T: InspectorName> CreateTypeData<T> for CustomNameInspectorImpl {
    fn create_type_data(_input: ()) -> Self {
        CustomNameInspectorImpl {
            fn_ptr: ui_read_name_vtable::<T>,
        }
    }
}
fn ui_read_name_vtable<T: InspectorName>(
    val: &dyn Any,
    ui: &mut egui::Ui,
    options: Option<&CustomAttributes>,
    id: egui::Id,
    env: &TypeRegistry,
) -> Response {
    let val = val.downcast_ref::<T>().unwrap();
    T::name_ui(val, ui, options, id, env)
}
fn ui_vtable<T: Inspector>(
    val: &mut dyn Any,
    ui: &mut egui::Ui,
    options: Option<&CustomAttributes>,
    id: egui::Id,
    env: &TypeRegistry,
) -> bool {
    let val = val.downcast_mut::<T>().unwrap();
    T::ui(val, ui, options, id, env)
}

impl<T: Inspector> CreateTypeData<T> for InspectorImpl {
    fn create_type_data(_input: ()) -> Self {
        InspectorImpl {
            fn_mut: ui_vtable::<T>,
        }
    }
}

/// The label a container drew for the value at an id, while that value is drawn.
#[derive(Clone)]
struct ValueLabel(egui::Id);

/// Names an input widget of the value drawn at `id`: after the visible label when
/// its container drew one, or generically when nothing labels it. An input with no
/// name can't be reached by a screen reader.
pub fn label_input(response: Response, id: egui::Id) -> Response {
    match response.ctx.data(|data| data.get_temp::<ValueLabel>(id)) {
        Some(ValueLabel(label)) => response.labelled_by(label),
        None => response.accessible_name("Value"),
    }
}

/// [`show`] for a value with a visible label next to it. The label is only known
/// during this call, because a label that isn't drawn this frame must not be
/// referenced: AccessKit panics on a dangling `labelled_by`.
fn show_labelled(
    type_reg: &TypeRegistry,
    ui: &mut egui::Ui,
    id: egui::Id,
    value: &mut dyn PartialReflect,
    options: Option<&CustomAttributes>,
    label: egui::Id,
) -> bool {
    ui.data_mut(|data| data.insert_temp(id, ValueLabel(label)));
    let changed = show(type_reg, ui, id, value, options);
    ui.data_mut(|data| data.remove::<ValueLabel>(id));
    changed
}

/// Draws an editor for `value` and returns whether it changed.
pub fn show(
    type_reg: &TypeRegistry,
    ui: &mut egui::Ui,
    id: egui::Id,
    value: &mut dyn PartialReflect,
    options: Option<&CustomAttributes>,
) -> bool {
    if let Some(data) = value
        .try_as_reflect_mut()
        .and_then(|r| type_reg.get_type_data::<InspectorImpl>(r.reflect_type_info().type_id()))
        && let Some(value) = value.try_as_reflect_mut()
    {
        return (data.fn_mut)(value.as_any_mut(), ui, options, id, type_reg);
    }
    match value.reflect_mut() {
        ReflectMut::Struct(value) => show_struct(type_reg, ui, id, value),
        ReflectMut::TupleStruct(tuple_struct) => show_tuple_struct(type_reg, ui, id, tuple_struct),
        ReflectMut::Tuple(tuple) => show_tuple(type_reg, ui, id, tuple),
        ReflectMut::List(list) => show_list(type_reg, ui, id, list),
        ReflectMut::Array(array) => show_array(type_reg, ui, id, array),
        ReflectMut::Map(map) => show_map(type_reg, ui, id, map),
        ReflectMut::Set(set) => show_set(type_reg, ui, id, set),
        ReflectMut::Enum(enum_value) => show_enum(type_reg, ui, id, enum_value),
        ReflectMut::Opaque(partial_reflect) => {
            ui.heading(partial_reflect.reflect_type_path());
            false
        }
    }
}

fn show_tuple(
    type_reg: &TypeRegistry,
    ui: &mut egui::Ui,
    id: egui::Id,
    tuple: &mut dyn Tuple,
) -> bool {
    let Some(bevy_reflect::TypeInfo::Tuple(type_info)) = tuple.get_represented_type_info() else {
        return false;
    };
    let mut changed = false;

    if type_info.field_len() < 2 {
        let Some(field) = tuple.field_mut(0) else {
            return false;
        };
        let Some(field_info) = type_info.field_at(0) else {
            return false;
        };
        return show(
            type_reg,
            ui,
            id,
            field,
            Some(field_info.custom_attributes()),
        );
    }
    let struct_margin = 14i8;
    egui::Grid::new(id.with("grid"))
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            for i in 0..tuple.field_len() {
                let Some(field_info) = type_info.field_at(i) else {
                    continue;
                };
                let Some(field) = tuple.field_mut(i) else {
                    continue;
                };

                let id = ui
                    .push_id(id.with(i), |ui| {
                        if let Some(data) =
                            type_reg.get_type_data::<CustomNameInspectorImpl>(field_info.type_id())
                            && let Some(f) = field.try_as_reflect()
                        {
                            return (data.fn_ptr)(
                                f.as_any(),
                                ui,
                                None,
                                id.with(i).with("name"),
                                type_reg,
                            );
                        }

                        ui.add(
                            egui::Label::new(RichText::new(i.to_string()).strong())
                                .selectable(false),
                        )
                        .add_tooltip(field_info.docs(), true)
                    })
                    .inner
                    .id;
                egui::Frame::new()
                    .outer_margin(Margin::symmetric(struct_margin, 0))
                    .show(ui, |ui| {
                        ui.with_layout(
                            Layout::top_down(Align::Min).with_cross_justify(true),
                            |ui| {
                                changed |= show_labelled(
                                    type_reg,
                                    ui,
                                    id.with("id").with(i),
                                    field,
                                    Some(field_info.custom_attributes()),
                                    id,
                                );
                            },
                        );
                    });
                ui.end_row();
            }
        });
    changed
}

fn show_array(
    type_reg: &TypeRegistry,
    ui: &mut egui::Ui,
    id: egui::Id,
    array: &mut dyn Array,
) -> bool {
    let Some(bevy_reflect::TypeInfo::Array(_)) = array.get_represented_type_info() else {
        return false;
    };
    let mut changed = false;
    egui::Grid::new(id.with("grid"))
        .striped(true)
        .num_columns(2)
        .spacing([0.0, 10.0])
        .show(ui, |ui| {
            for i in 0..array.len() {
                let label = egui::Frame::new()
                    .inner_margin(Margin {
                        left: 10,
                        right: 10,
                        top: 0,
                        bottom: 0,
                    })
                    .show(ui, |ui| {
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(format!("[{i}]"))).selectable(false),
                            )
                            .id
                        })
                        .inner
                    })
                    .inner;
                if let Some(value) = array.get_mut(i) {
                    ui.with_layout(
                        Layout::top_down(Align::Min).with_cross_justify(true),
                        |ui| {
                            changed |= show_labelled(type_reg, ui, id.with(i), value, None, label);
                        },
                    );
                };
                ui.end_row();
            }
        });
    changed
}

/// `field_name` as `Field Name`, for labels.
pub fn snake_to_title(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn show_tuple_struct(
    type_reg: &TypeRegistry,
    ui: &mut egui::Ui,
    id: egui::Id,
    value: &mut dyn TupleStruct,
) -> bool {
    let Some(bevy_reflect::TypeInfo::TupleStruct(type_info)) = value.get_represented_type_info()
    else {
        return false;
    };
    let mut changed = false;

    if type_info.field_len() < 2 {
        let Some(field) = value.field_mut(0) else {
            return false;
        };
        let Some(field_info) = type_info.field_at(0) else {
            return false;
        };
        return show(
            type_reg,
            ui,
            id,
            field,
            Some(field_info.custom_attributes()),
        );
    }
    let struct_margin = 14i8;
    egui::Grid::new(id.with("grid"))
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            for i in 0..value.field_len() {
                let Some(field_info) = type_info.field_at(i) else {
                    continue;
                };
                let Some(field) = value.field_mut(i) else {
                    continue;
                };

                let id = ui
                    .push_id(id.with(i), |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(i.to_string()).strong())
                                .selectable(false),
                        )
                        .add_tooltip(field_info.docs(), true)
                    })
                    .inner
                    .id;
                egui::Frame::new()
                    .outer_margin(Margin::symmetric(struct_margin, 0))
                    .show(ui, |ui| {
                        ui.with_layout(
                            Layout::top_down(Align::Min).with_cross_justify(true),
                            |ui| {
                                changed |= show_labelled(
                                    type_reg,
                                    ui,
                                    id.with("id").with(i),
                                    field,
                                    Some(field_info.custom_attributes()),
                                    id,
                                );
                            },
                        );
                    });
                ui.end_row();
            }
        });
    changed
}
pub fn show_struct(
    type_reg: &TypeRegistry,
    ui: &mut egui::Ui,
    id: egui::Id,
    value: &mut dyn Struct,
) -> bool {
    let Some(bevy_reflect::TypeInfo::Struct(type_info)) = value.get_represented_type_info() else {
        return false;
    };
    let mut changed = false;
    egui::Grid::new(id.with("grid"))
        .striped(true)
        .num_columns(2)
        .spacing([0.0, 10.0])
        .show(ui, |ui| {
            for i in 0..value.field_len() {
                let Some(field_info) = type_info.field_at(i) else {
                    continue;
                };

                let Some(field) = value.field_at_mut(i) else {
                    continue;
                };

                let label = egui::Frame::new()
                    .inner_margin(Margin {
                        left: 10,
                        right: 10,
                        top: 0,
                        bottom: 0,
                    })
                    .show(ui, |ui| {
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(snake_to_title(field_info.name())))
                                    .selectable(false),
                            )
                            .add_tooltip(field_info.docs(), true)
                            .id
                        })
                        .inner
                    })
                    .inner;
                ui.with_layout(
                    Layout::top_down(Align::Center)
                        .with_cross_justify(true)
                        .with_main_justify(true),
                    |ui| {
                        changed |= show_labelled(
                            type_reg,
                            ui,
                            id.with(field_info.name()),
                            field,
                            Some(field_info.custom_attributes()),
                            label,
                        );
                    },
                );
                ui.end_row();
            }
        });

    changed
}

/// The frame around a list, map or set.
fn container_frame(ui: &egui::Ui) -> egui::Frame {
    let palette = Palette::of(ui);
    egui::Frame::default()
        .stroke(Stroke::new(1.0, palette.border_subtle))
        .fill(palette.extreme_bg)
        .corner_radius(Metrics::of(ui).card_radius)
        .inner_margin(8i8)
}

enum ListOp {
    AddElement(usize),
    RemoveElement(usize),
    MoveElementUp(usize),
    MoveElementDown(usize),
    MoveFromTo(usize, usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ListDrag {
    index: usize,
    id: egui::Id,
}

pub fn show_list(
    type_reg: &TypeRegistry,
    ui: &mut egui::Ui,
    id: egui::Id,
    list: &mut dyn List,
) -> bool {
    let mut changed = false;
    container_frame(ui).show(ui, |ui| {
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
            let mut op = None;
            let len = list.len();
            ui.with_layout(Layout::left_to_right(egui::Align::Min), |ui| {
                ui.add(
                    egui::Label::new(RichText::new(format!("{} elements", len)))
                        .extend()
                        .selectable(false),
                );

                ui.add_space(10.0);
                let available_space = ui.available_size();

                ui.allocate_ui_with_layout(
                    available_space,
                    Layout::right_to_left(egui::Align::Min),
                    |ui| {
                        if ui
                            .add(egui::Button::new("+"))
                            .on_hover_text("Add new element")
                            .clicked()
                        {
                            op = Some(ListOp::AddElement(0));
                        }
                    },
                );
            });
            ui.separator();

            let palette = Palette::of(ui);
            let frame = egui::Frame::default()
                .stroke(Stroke::new(1.0, palette.border))
                .fill(palette.extreme_bg)
                .inner_margin(Margin {
                    left: 20,
                    right: 5,
                    top: 5,
                    bottom: 5,
                });

            for i in 0..len {
                let (_, dropped_payload) = ui.dnd_drop_zone::<ListDrag, ()>(frame, |ui| {
                    ui.horizontal(|ui| {
                        // One button for both: a click opens the menu and a drag
                        // reorders. `dnd_drag_source` can't wrap it, because a
                        // drag-only widget on top takes the clicks from what's below.
                        let index_button =
                            egui::Button::new(i.to_string()).sense(egui::Sense::click_and_drag());
                        let (response, _) = egui::containers::menu::MenuButton::from_button(
                            index_button,
                        )
                        .ui(ui, |ui| {
                            if ui
                                .button("Remove")
                                .on_hover_text("Remove element")
                                .clicked()
                            {
                                op = Some(ListOp::RemoveElement(i));
                            }
                            let up_enabled = i > 0;
                            ui.add_enabled_ui(up_enabled, |ui| {
                                if ui
                                    .button("Move up")
                                    .on_hover_text("Move element up")
                                    .clicked()
                                {
                                    op = Some(ListOp::MoveElementUp(i));
                                }
                            });
                            let down_enabled = len.checked_sub(1).map(|l| i < l).unwrap_or(false);
                            ui.add_enabled_ui(down_enabled, |ui| {
                                if ui
                                    .button("Move down")
                                    .on_hover_text("Move element down")
                                    .clicked()
                                {
                                    op = Some(ListOp::MoveElementDown(i));
                                }
                            });
                        });
                        let response = response.on_hover_cursor(egui::CursorIcon::Grab);
                        if response.drag_started() {
                            egui::DragAndDrop::set_payload(ui.ctx(), ListDrag { index: i, id });
                        }
                        ui.add_space(10.0);
                        ui.vertical_centered_justified(|ui| {
                            let val = list.get_mut(i).unwrap();
                            changed |= show(type_reg, ui, id.with(i), val, None);
                        });
                        // Detect drops onto this item:
                        if let (Some(pointer), Some(hovered_payload)) = (
                            ui.input(|i| i.pointer.interact_pos()),
                            response.dnd_hover_payload::<ListDrag>(),
                        ) && hovered_payload.id.eq(&id)
                        {
                            let rect = response.rect;

                            // Preview insertion:
                            let stroke = Stroke::new(1.0, palette.accent);
                            if hovered_payload.index == i {
                                // We are dragged onto ourselves
                                ui.painter().hline(rect.x_range(), rect.center().y, stroke);
                            } else if pointer.y < rect.center().y {
                                // Above us
                                ui.painter().hline(rect.x_range(), rect.top(), stroke);
                            } else {
                                // Below us
                                ui.painter().hline(rect.x_range(), rect.bottom(), stroke);
                            }
                        }
                    });
                });

                if let Some(dragged_payload) = dropped_payload
                    && dragged_payload.id.eq(&id)
                {
                    op = Some(ListOp::MoveFromTo(dragged_payload.index, i));
                }
            }

            let Some(TypeInfo::List(info)) = list.get_represented_type_info() else {
                return;
            };
            // Respond to control interaction
            if let Some(op) = op {
                match op {
                    ListOp::AddElement(i) => {
                        let default = get_default_value_for(type_reg, info.item_ty().id())
                            .map(|def| def.into_partial_reflect())
                            .or_else(|| list.get(i).and_then(|v| v.to_dynamic().ok()));
                        if let Some(new_value) = default {
                            list.insert(i, new_value);
                            changed = true;
                        }
                    }
                    ListOp::RemoveElement(i) => {
                        list.remove(i);
                        changed = true;
                    }
                    ListOp::MoveElementUp(i) => {
                        // Clone this element and insert it at its index - 1.
                        if let Some(prev_idx) = i.checked_sub(1)
                            && let Some(clone) = list.get(i).and_then(|e| e.to_dynamic().ok())
                        {
                            list.insert(prev_idx, clone);
                            // Remove the original, now at its index + 1.
                            list.remove(i + 1);
                            changed = true;
                        }
                    }
                    ListOp::MoveElementDown(i) => {
                        // Clone the next element and insert it at this index.
                        if let Some(next_clone) = list.get(i + 1).and_then(|e| e.to_dynamic().ok())
                        {
                            list.insert(i, next_clone);
                            // Remove the original, now at i + 2.
                            list.remove(i + 2);
                            changed = true;
                        }
                    }
                    ListOp::MoveFromTo(from, to) => {
                        let element = list.remove(from);
                        if list.len() < to {
                            list.push(element);
                        } else {
                            list.insert(to, element);
                        }
                        changed = true;
                    }
                }
            }
        });
    });

    changed
}

pub fn show_map(
    type_reg: &TypeRegistry,
    ui: &mut egui::Ui,
    id: egui::Id,
    map: &mut dyn Map,
) -> bool {
    let mut changed = false;
    container_frame(ui).show(ui, |ui| {
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
            let len = map.len();
            ui.add(
                egui::Label::new(RichText::new(format!("{} entries", len)))
                    .extend()
                    .selectable(false),
            );

            egui::Grid::new(id.with("grid"))
                .striped(true)
                .num_columns(1)
                .show(ui, |ui| {
                    ui.vertical_centered_justified(|ui| {
                        map.retain(&mut |key, value| {
                            let mut remove = false;
                            let key_v = format!("{:?}", key);

                            egui::CollapsingHeader::new(&key_v).show(ui, |ui| {
                                changed |= show(
                                    type_reg,
                                    ui,
                                    id.with(key_v),
                                    value.as_partial_reflect_mut(),
                                    None,
                                );

                                ui.add_space(5.0);
                                if ui.button("Remove").clicked() {
                                    remove = true;
                                    changed = true;
                                }
                                ui.add_space(5.0);
                            });
                            !remove
                        });
                    });
                    ui.end_row();
                });
        });
    });
    changed
}

pub fn show_set(
    _type_reg: &TypeRegistry,
    ui: &mut egui::Ui,
    id: egui::Id,
    set: &mut dyn Set,
) -> bool {
    let changed = false;
    container_frame(ui).show(ui, |ui| {
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
            let len = set.len();
            ui.with_layout(Layout::left_to_right(egui::Align::Min), |ui| {
                ui.add(
                    egui::Label::new(RichText::new(format!("{} elements", len)))
                        .extend()
                        .selectable(false),
                );
            });

            if len > 0 {
                egui::Grid::new(id.with("grid"))
                    .striped(true)
                    .num_columns(1)
                    .show(ui, |ui| {
                        for value in set.iter() {
                            ui.label(format!("{:?}", value.reflect_type_path()));
                            ui.end_row();
                        }
                    });
            } else {
                ui.label("Set is empty");
            }
        });
    });
    changed
}

pub fn show_enum(
    type_reg: &TypeRegistry,
    ui: &mut egui::Ui,
    id: egui::Id,
    value: &mut dyn Enum,
) -> bool {
    let Some(bevy_reflect::TypeInfo::Enum(type_info)) = value.get_represented_type_info() else {
        return false;
    };
    let mut changed = false;
    let active_variant_idx = value.variant_index();
    let (columns, add_label) = if value.field_len() > 1 {
        (2, true)
    } else {
        (1, false)
    };
    if is_option_type(type_info) {
        egui::Grid::new(id.with("OPTIONAL"))
            .num_columns(2)
            .show(ui, |ui| {
                let mut enabled = active_variant_idx == 1;
                let checkbox = ui.checkbox(&mut enabled, ()).on_hover_text("Optional type");
                if label_input(checkbox, id).clicked()
                    && let Ok(new_val) = construct_default_variant(
                        type_reg,
                        type_info.variant_at(if enabled { 1 } else { 0 }).unwrap(),
                    )
                {
                    value.apply(&new_val);
                    changed = true;
                }
                ui.with_layout(
                    Layout::top_down(Align::Min).with_cross_justify(true),
                    |ui| {
                        for i in 0..value.field_len() {
                            if let Some(field) = value.field_at_mut(i) {
                                changed |= show(type_reg, ui, id.with(i), field, None);
                            }
                        }
                    },
                );
            });
    } else if all_unit_variants(type_info) && type_info.variant_len() < 3 {
        ui.vertical_centered_justified(|ui| {
            let width = (ui.available_width() / type_info.variant_len() as f32 - 20.0).max(10.0);
            ui.btn_frame().show(ui, |ui| {
                ui.scope_builder(ui.btn_frame_ui(), |ui| {
                    let spacing = ui.spacing().item_spacing;
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                    let total = type_info.variant_len();
                    let radius = Metrics::of(ui).control_radius;
                    for (i, variant) in type_info.iter().enumerate() {
                        let variant_name = variant.name();
                        let is_active_variant = i == active_variant_idx;
                        let can_construct = variant_constructable(type_reg, variant).is_ok();
                        let clicked = ui
                            .add_enabled(
                                can_construct,
                                egui::Button::selectable(is_active_variant, variant_name)
                                    .min_size([width, 15.0].into())
                                    .corner_radius(
                                        ButtonGroupElement::from_index(i, total)
                                            .to_corner_radius(radius),
                                    ),
                            )
                            .clicked();
                        if clicked && let Ok(new_val) = construct_default_variant(type_reg, variant)
                        {
                            changed = true;
                            value.apply(&new_val);
                        }
                    }
                    ui.spacing_mut().item_spacing = spacing;
                });
            });
        });
    } else {
        ui.vertical_centered_justified(|ui| {
            let width = ui.available_width();
            let response = egui::ComboBox::from_id_salt(id.with("select"))
                .width(width)
                .wrap_mode(egui::TextWrapMode::Extend)
                .selected_text(type_info.variant_names()[active_variant_idx])
                .show_ui(ui, |ui| {
                    for (i, variant) in type_info.iter().enumerate() {
                        let variant_name = variant.name();
                        let is_active_variant = i == active_variant_idx;
                        let can_construct = variant_constructable(type_reg, variant).is_ok();
                        let clicked = ui
                            .add_enabled(
                                can_construct,
                                egui::Button::selectable(is_active_variant, variant_name),
                            )
                            .clicked();
                        if clicked && let Ok(new_val) = construct_default_variant(type_reg, variant)
                        {
                            changed = true;
                            value.apply(&new_val);
                        }
                    }
                })
                .response;
            label_input(response, id);
            let enum_padding_x = 0.0;
            let enum_padding_y = 40.0;
            if value.field_len() == 0 {
                return;
            }
            egui::Grid::new(id.with("struct"))
                .striped(true)
                .num_columns(columns)
                .spacing([enum_padding_x, enum_padding_y])
                .show(ui, |ui| {
                    for i in 0..value.field_len() {
                        let label = add_label.then(|| match value.name_at(i) {
                            Some(name) => ui.label(name).id,
                            None => ui.label(i.to_string()).id,
                        });
                        if let Some(field) = value.field_at_mut(i) {
                            ui.with_layout(
                                egui::Layout::top_down_justified(egui::Align::Min),
                                |ui| {
                                    changed |= match label {
                                        Some(label) => show_labelled(
                                            type_reg,
                                            ui,
                                            id.with(i),
                                            field,
                                            None,
                                            label,
                                        ),
                                        None => show(type_reg, ui, id.with(i), field, None),
                                    };
                                },
                            );
                        }
                        ui.end_row();
                    }
                });
        });
    }

    changed
}

fn variant_constructable<'a>(
    type_registry: &TypeRegistry,
    variant: &'a VariantInfo,
) -> Result<(), Vec<&'a str>> {
    let type_id_is_constructable = |type_id: TypeId| {
        type_registry
            .get_type_data::<ReflectDefault>(type_id)
            .is_some()
    };

    let unconstructable_fields: Vec<&'a str> = match variant {
        VariantInfo::Struct(variant) => variant
            .iter()
            .filter_map(|field| {
                (!type_id_is_constructable(field.type_id())).then_some(field.type_path())
            })
            .collect(),
        VariantInfo::Tuple(variant) => variant
            .iter()
            .filter_map(|field| {
                (!type_id_is_constructable(field.type_id())).then_some(field.type_path())
            })
            .collect(),
        VariantInfo::Unit(_) => return Ok(()),
    };

    if unconstructable_fields.is_empty() {
        Ok(())
    } else {
        Err(unconstructable_fields)
    }
}

fn construct_default_variant(
    type_reg: &TypeRegistry,
    variant: &VariantInfo,
) -> Result<DynamicEnum, ()> {
    let dynamic_variant = match variant {
        VariantInfo::Struct(struct_info) => {
            let mut dynamic_struct = DynamicStruct::default();
            for field in struct_info.iter() {
                let field_default_value = match get_default_value_for(type_reg, field.type_id()) {
                    Some(value) => value,
                    None => {
                        return Err(());
                    }
                };
                dynamic_struct.insert_boxed(
                    field.name(),
                    field_default_value.to_dynamic().map_err(|_| ())?,
                );
            }
            DynamicVariant::Struct(dynamic_struct)
        }
        VariantInfo::Tuple(tuple_info) => {
            let mut dynamic_tuple = DynamicTuple::default();
            for field in tuple_info.iter() {
                let field_default_value = match get_default_value_for(type_reg, field.type_id()) {
                    Some(value) => value,
                    None => {
                        return Err(());
                    }
                };
                dynamic_tuple.insert_boxed(field_default_value.to_dynamic().map_err(|_| ())?);
            }
            DynamicVariant::Tuple(dynamic_tuple)
        }
        VariantInfo::Unit(_) => DynamicVariant::Unit,
    };
    let dynamic_enum = DynamicEnum::new(variant.name(), dynamic_variant);
    Ok(dynamic_enum)
}

fn get_default_value_for(type_reg: &TypeRegistry, type_id: TypeId) -> Option<Box<dyn Reflect>> {
    if let Some(reflect_default) = type_reg.get_type_data::<ReflectDefault>(type_id) {
        return Some(reflect_default.default());
    }

    None
}
