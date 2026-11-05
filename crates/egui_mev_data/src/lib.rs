//! Forms and tables described as data, so an app can read them from JSON or a
//! plugin instead of writing a screen for each.
//!
//! A [`Form`] is checked, drawn and validated here; a [`TableView`] sorts and
//! filters its rows and hands back the clicked row's payload, which is whatever
//! type the app gave the row.

pub mod form;
pub mod table;

pub use form::{
    ChoiceOption, Condition, FieldDef, FieldType, Form, FormState, Outcome, PathMode, PathRow,
    SubmitVariant, Values, format_time, parse_time,
};
pub use table::{
    Cell, ColumnDef, ColumnKind, RowAction, RowClick, RowDef, RowStatus, RowUpdate, SizeFilter,
    SizeUnit, SortDef, SortOrder, TableLabels, TableSpec, TableView,
};
