//! Drives the editors through their widgets and checks the reflected value.

use std::collections::HashMap;

use bevy_reflect::{
    PartialReflect, Reflect, TypeRegistry, attributes::CustomAttributes, std_traits::ReflectDefault,
};
use egui::{Id, Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT as _, Queryable as _},
};
use egui_mev_inspect::{Inspector, InspectorImpl, create_registry, show, snake_to_title};

struct Editor<T> {
    value: T,
    registry: TypeRegistry,
    changed: bool,
}

impl<T> Editor<T> {
    /// Whether an edit was reported since the last call.
    fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }
}

fn harness_with<T: PartialReflect>(
    value: T,
    registry: TypeRegistry,
) -> Harness<'static, Editor<T>> {
    let mut harness = Harness::builder().with_size([500.0, 500.0]).build_ui_state(
        |ui, editor: &mut Editor<T>| {
            editor.changed |= show(
                &editor.registry,
                ui,
                Id::unique("editing test"),
                &mut editor.value,
                None,
            );
        },
        Editor {
            value,
            registry,
            changed: false,
        },
    );
    harness.run();
    harness
}

fn harness<T: PartialReflect>(value: T) -> Harness<'static, Editor<T>> {
    harness_with(value, create_registry())
}

/// Replaces the number in the first drag value on screen. A click starts editing
/// with all of its text selected, and it keeps its role while being edited.
fn type_number<T>(harness: &mut Harness<'_, Editor<T>>, text: &str) {
    let first = |harness: &Harness<'_, Editor<T>>| {
        let drag_value = harness.get_all_by_role(Role::SpinButton).next();
        drag_value.expect("should show a drag value").rect()
    };
    let rect = first(harness);
    harness.hover_at(rect.center());
    harness.drag_at(rect.center());
    harness.drop_at(rect.center());
    harness.run();
    harness
        .query_by(|node| node.is_focused())
        .expect("clicking should focus the drag value")
        .type_text(text);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
}

#[derive(Debug, Default, Clone, PartialEq, Reflect)]
struct Person {
    /// Name of the person.
    full_name: String,
    age: u32,
    active: bool,
}

#[test]
fn struct_labels_are_titled() {
    let harness = harness(Person::default());

    harness.get_by_label("Full Name");
    harness.get_by_label("Age");
    harness.get_by_label("Active");
}

#[test]
fn struct_string_field_edits_the_value() {
    let mut harness = harness(Person::default());

    harness.get_by_role(Role::TextInput).click();
    harness.run();
    harness.get_by_role(Role::TextInput).type_text("Ada");
    harness.run();

    assert_eq!(harness.state().value.full_name, "Ada");
    assert!(harness.state_mut().take_changed());
}

#[test]
fn struct_number_field_edits_the_value() {
    let mut harness = harness(Person::default());

    type_number(&mut harness, "42");

    assert_eq!(harness.state().value.age, 42);
    assert!(harness.state_mut().take_changed());
}

#[test]
fn struct_bool_field_edits_the_value() {
    let mut harness = harness(Person::default());

    harness.get_by_role(Role::CheckBox).click();
    harness.run();

    assert!(harness.state().value.active);
    assert!(harness.state_mut().take_changed());
}

#[test]
fn drawing_alone_reports_no_change() {
    let mut harness = harness(Person::default());

    harness.run_steps(3);

    assert_eq!(harness.state().value, Person::default());
    assert!(!harness.state_mut().take_changed());
}

#[derive(Debug, Clone, PartialEq, Reflect)]
enum Toggle {
    Off,
    On,
}

#[test]
fn enum_of_two_unit_variants_switches_with_buttons() {
    let mut harness = harness(Toggle::Off);

    harness.get_by_label("On").click();
    harness.run();

    assert_eq!(harness.state().value, Toggle::On);
    assert!(harness.state_mut().take_changed());
}

#[derive(Debug, Clone, PartialEq, Reflect)]
enum Shape {
    Empty,
    Circle { radius: f32 },
    Label(String),
}

#[test]
fn enum_switches_to_a_struct_variant_with_defaults() {
    let mut harness = harness(Shape::Empty);

    harness.get_by_role(Role::ComboBox).click();
    harness.run();
    harness.get_by_label("Circle").click();
    harness.run();

    assert_eq!(harness.state().value, Shape::Circle { radius: 0.0 });
    assert!(harness.state_mut().take_changed());
}

#[test]
fn enum_switches_to_a_tuple_variant_and_its_field_edits() {
    let mut harness = harness(Shape::Empty);

    harness.get_by_role(Role::ComboBox).click();
    harness.run();
    harness.get_by_label("Label").click();
    harness.run();
    assert_eq!(harness.state().value, Shape::Label(String::new()));

    harness.get_by_role(Role::TextInput).click();
    harness.run();
    harness.get_by_role(Role::TextInput).type_text("hi");
    harness.run();

    assert_eq!(harness.state().value, Shape::Label("hi".to_owned()));
}

#[derive(Debug, Default, Clone, PartialEq, Reflect)]
struct Unregistered {
    x: u32,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
enum NeedsDefault {
    Plain,
    Nested(Unregistered),
    Other,
}

#[test]
fn enum_variant_without_a_default_stays_disabled() {
    let mut harness = harness(NeedsDefault::Plain);

    harness.get_by_role(Role::ComboBox).click();
    harness.run();
    harness.get_by_label("Nested").click();
    harness.run();

    assert_eq!(harness.state().value, NeedsDefault::Plain);
    assert!(!harness.state_mut().take_changed());
}

#[test]
fn option_checkbox_switches_between_none_and_some() {
    let mut harness = harness(None::<u32>);

    harness.get_by_role(Role::CheckBox).click();
    harness.run();
    assert_eq!(harness.state().value, Some(0));
    assert!(harness.state_mut().take_changed());

    harness.get_by_role(Role::CheckBox).click();
    harness.run();
    assert_eq!(harness.state().value, None);
    assert!(harness.state_mut().take_changed());
}

#[test]
fn list_adds_a_default_element() {
    let mut harness = harness(vec!["old".to_owned()]);

    harness.get_by_label("+").click();
    harness.run();

    assert_eq!(harness.state().value, ["", "old"]);
    assert!(harness.state_mut().take_changed());
    harness.get_by_label("2 elements");
}

#[test]
fn empty_list_adds_a_default_element() {
    let mut harness = harness(Vec::<u32>::new());

    harness.get_by_label("+").click();
    harness.run();

    assert_eq!(harness.state().value, [0]);
}

#[test]
fn list_without_a_default_copies_its_first_element() {
    let mut harness = harness(vec![Unregistered { x: 7 }]);

    harness.get_by_label("+").click();
    harness.run();

    assert_eq!(
        harness.state().value,
        [Unregistered { x: 7 }, Unregistered { x: 7 }]
    );
}

#[test]
fn empty_list_without_a_default_reports_no_change() {
    let mut harness = harness(Vec::<Unregistered>::new());

    harness.get_by_label("+").click();
    harness.run();

    assert!(harness.state().value.is_empty());
    assert!(!harness.state_mut().take_changed());
}

#[test]
fn list_uses_a_default_from_a_registered_type() {
    let mut registry = create_registry();
    registry.register::<Registered>();
    let mut harness = harness_with(Vec::<Registered>::new(), registry);

    harness.get_by_label("+").click();
    harness.run();

    assert_eq!(harness.state().value, [Registered { x: 5 }]);
}

#[derive(Debug, Clone, PartialEq, Reflect)]
#[reflect(Default)]
struct Registered {
    x: u32,
}

impl Default for Registered {
    fn default() -> Self {
        Self { x: 5 }
    }
}

/// Opens the menu of the element at `index` and clicks `action` in it.
fn list_menu<T>(harness: &mut Harness<'_, Editor<T>>, index: usize, action: &str) {
    harness.get_by_label(&index.to_string()).click();
    harness.run();
    harness.get_by_label(action).click();
    harness.run();
}

#[test]
fn list_removes_an_element() {
    let mut harness = harness(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);

    list_menu(&mut harness, 1, "Remove");

    assert_eq!(harness.state().value, ["a", "c"]);
    assert!(harness.state_mut().take_changed());
}

#[test]
fn list_moves_an_element_up_and_down() {
    let mut harness = harness(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);

    list_menu(&mut harness, 1, "Move up");
    assert_eq!(harness.state().value, ["b", "a", "c"]);

    list_menu(&mut harness, 1, "Move down");
    assert_eq!(harness.state().value, ["b", "c", "a"]);
}

/// Drags the index button of element `from` onto the row of element `to`.
fn drag_element<T>(harness: &mut Harness<'_, Editor<T>>, from: usize, to: usize) {
    let start = harness.get_by_label(&from.to_string()).rect().center();
    let end = harness.get_by_label(&to.to_string()).rect().center();
    harness.hover_at(start);
    harness.run();
    harness.drag_at(start);
    harness.run();
    harness.hover_at(end);
    harness.run();
    harness.drop_at(end);
    harness.run();
}

#[test]
fn list_drag_moves_an_element_forward() {
    let mut harness = harness(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);

    drag_element(&mut harness, 0, 2);

    assert_eq!(harness.state().value, ["b", "c", "a"]);
    assert!(harness.state_mut().take_changed());
}

#[test]
fn list_drag_moves_an_element_back() {
    let mut harness = harness(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);

    drag_element(&mut harness, 2, 0);

    assert_eq!(harness.state().value, ["c", "a", "b"]);
}

#[test]
fn list_element_edits_the_value() {
    let mut harness = harness(vec![1_u32]);

    type_number(&mut harness, "9");

    assert_eq!(harness.state().value, [9]);
    assert!(harness.state_mut().take_changed());
}

fn scores() -> HashMap<String, u32> {
    HashMap::from([("alice".to_owned(), 1), ("bob".to_owned(), 2)])
}

#[test]
fn map_value_edits_the_entry() {
    let mut harness = harness(scores());
    harness.get_by_label("2 entries");

    harness.get_by_label("\"alice\"").click();
    harness.run();
    type_number(&mut harness, "50");

    assert_eq!(harness.state().value["alice"], 50);
    assert_eq!(harness.state().value["bob"], 2);
    assert!(harness.state_mut().take_changed());
}

#[test]
fn map_remove_drops_only_that_entry_and_reports_it() {
    let mut harness = harness(scores());

    harness.get_by_label("\"bob\"").click();
    harness.run();
    harness.get_by_label("Remove").click();
    harness.run();

    assert_eq!(
        harness.state().value,
        HashMap::from([("alice".to_owned(), 1)])
    );
    assert!(harness.state_mut().take_changed());
    harness.get_by_label("1 entries");
}

#[derive(Debug, Default, Clone, PartialEq, Reflect)]
struct Counter(u32, u32);

#[test]
fn tuple_struct_field_edits_the_value() {
    let mut harness = harness(Counter(1, 2));

    type_number(&mut harness, "8");

    assert_eq!(harness.state().value, Counter(8, 2));
}

#[derive(Debug, Default, Clone, PartialEq, Reflect)]
struct Custom {
    clicks: u32,
}

impl Inspector for Custom {
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _: Option<&CustomAttributes>,
        _: Id,
        _: &TypeRegistry,
    ) -> bool {
        let clicked = ui.button("Count").clicked();
        self.clicks += u32::from(clicked);
        clicked
    }
}

#[test]
fn registered_inspector_replaces_the_default_editor() {
    let mut registry = create_registry();
    registry.register::<Custom>();
    registry.register_type_data::<Custom, InspectorImpl>();
    let mut harness = harness_with(Custom::default(), registry);

    harness.get_by_label("Count").click();
    harness.run();

    assert_eq!(harness.state().value.clicks, 1);
    assert!(harness.state_mut().take_changed());
}

#[test]
fn registry_has_editors_for_the_types_it_promises() {
    let registry = create_registry();

    for type_id in [
        std::any::TypeId::of::<std::num::NonZeroU16>(),
        std::any::TypeId::of::<std::num::NonZeroU32>(),
        std::any::TypeId::of::<std::num::NonZeroU64>(),
        std::any::TypeId::of::<std::time::Duration>(),
        std::any::TypeId::of::<std::borrow::Cow<'static, str>>(),
        std::any::TypeId::of::<String>(),
    ] {
        assert!(registry.get_type_data::<InspectorImpl>(type_id).is_some());
    }
}

/// The harness panics on an unnamed input and on a label that isn't drawn, so these
/// pass only when each value's widgets are named by a label on screen.
#[test]
fn every_input_is_named_after_the_label_beside_it() {
    let mut harness = harness(Person::default());

    let name = harness.get_by_role(Role::TextInput);
    assert_eq!(name.accesskit_node().label().as_deref(), Some("Full Name"));
    let age = harness.get_by_role(Role::SpinButton);
    assert_eq!(age.accesskit_node().label().as_deref(), Some("Age"));
    let active = harness.get_by_role(Role::CheckBox);
    assert_eq!(active.accesskit_node().label().as_deref(), Some("Active"));
    harness.run_steps(2);
}

#[derive(Debug, Clone, PartialEq, Reflect)]
enum FieldCounts {
    Two { first: u32, second: u32 },
    One(u32),
}

/// `Two` labels the field at index 0 and `One` doesn't, on the same id: a label left
/// over from the previous variant would dangle and panic in AccessKit.
#[test]
fn switching_to_a_variant_without_labels_leaves_no_stale_label() {
    let mut harness = harness(FieldCounts::Two {
        first: 1,
        second: 2,
    });
    let first = harness.get_all_by_role(Role::SpinButton).next();
    let first = first.expect("should show a drag value");
    assert_eq!(first.accesskit_node().label().as_deref(), Some("first"));

    harness.get_by_role(Role::ComboBox).click();
    harness.run();
    harness.get_by_label("One").click();
    harness.run();
    harness.run_steps(3);

    assert_eq!(harness.state().value, FieldCounts::One(0));
    let only = harness.get_by_role(Role::SpinButton);
    assert_eq!(only.accesskit_node().label().as_deref(), Some("Value"));
}

#[test]
fn value_with_no_label_gets_a_generic_name() {
    let harness = harness(3_u32);

    let input = harness.get_by_role(Role::SpinButton);
    assert_eq!(input.accesskit_node().label().as_deref(), Some("Value"));
}

#[test]
fn snake_to_title_capitalizes_each_word() {
    assert_eq!(snake_to_title("field_name"), "Field Name");
    assert_eq!(snake_to_title("word"), "Word");
    assert_eq!(snake_to_title("Already_Title"), "Already Title");
    assert_eq!(snake_to_title("x2_y"), "X2 Y");
    assert_eq!(snake_to_title("żółć_ćma"), "Żółć Ćma");
}

#[test]
fn snake_to_title_keeps_the_gaps_of_empty_words() {
    assert_eq!(snake_to_title(""), "");
    assert_eq!(snake_to_title("_"), " ");
    assert_eq!(snake_to_title("a__b"), "A  B");
}
