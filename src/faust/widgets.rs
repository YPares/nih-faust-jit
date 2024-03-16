use super::wrapper::*;
use std::{
    collections::VecDeque,
    ffi::{c_void, CStr},
};

#[derive(Debug, PartialEq, Eq)]
pub enum BoxLayout {
    Horizontal,
    Vertical,
}

impl BoxLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::HORIZONTAL_BOX => Self::Horizontal,
            WWidgetDeclType::VERTICAL_BOX => Self::Vertical,
            _ => panic!("Not a BoxLayout"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ButtonLayout {
    Trigger,
    Checkbox,
}

impl ButtonLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::BUTTON => Self::Trigger,
            WWidgetDeclType::CHECK_BUTTON => Self::Checkbox,
            _ => panic!("Not a ButtonLayout"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum NumericLayout {
    NumEntry,
    HorizontalSlider,
    VerticalSlider,
}

impl NumericLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::NUM_ENTRY => Self::NumEntry,
            WWidgetDeclType::HORIZONTAL_SLIDER => Self::HorizontalSlider,
            WWidgetDeclType::VERTICAL_SLIDER => Self::VerticalSlider,
            _ => panic!("Not a NumericLayout"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum BargraphLayout {
    Horizontal,
    Vertical,
}

impl BargraphLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::HORIZONTAL_BARGRAPH => Self::Horizontal,
            WWidgetDeclType::VERTICAL_BARGRAPH => Self::Vertical,
            _ => panic!("Not a BargraphLayout"),
        }
    }
}

/// Lifetime 'a corresponds to that of the dsp object
#[derive(Debug)]
pub enum DspWidget<'a> {
    TabGroup {
        label: String,
        inner: Vec<DspWidget<'a>>,
        selected: usize,
    },
    Box {
        layout: BoxLayout,
        label: String,
        inner: Vec<DspWidget<'a>>,
    },
    Button {
        layout: ButtonLayout,
        label: String,
        zone: &'a mut f32,
    },
    Numeric {
        layout: NumericLayout,
        label: String,
        zone: &'a mut f32,
        init: f32,
        min: f32,
        max: f32,
        step: f32,
    },
    Bargraph {
        layout: BargraphLayout,
        label: String,
        zone: &'a mut f32,
        min: f32,
        max: f32,
    },
    // Soundfile {
    //     label: String,
    //     path: PathBuf,
    //     // TODO
    // },
}

impl<'a> DspWidget<'a> {
    fn inner_mut(&mut self) -> Option<&mut Vec<DspWidget<'a>>> {
        match self {
            DspWidget::TabGroup { inner, .. } => Some(inner),
            DspWidget::Box { inner, .. } => Some(inner),
            _ => None,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            DspWidget::TabGroup { label, .. } => label,
            DspWidget::Box { label, .. } => label,
            DspWidget::Button { label, .. } => label,
            DspWidget::Numeric { label, .. } => label,
            DspWidget::Bargraph { label, .. } => label,
        }
    }
}

pub struct DspWidgetsBuilder {
    widget_decls: VecDeque<WWidgetDecl>,
}

impl DspWidgetsBuilder {
    pub fn new() -> Self {
        Self {
            widget_decls: VecDeque::new(),
        }
    }

    /// To be called _after_ faust's buildUserInterface has finished, ie. after
    /// w_createUIs has finished. 'a is the lifetime of the DSP itself
    pub fn build_widgets<'a>(&mut self, cur_level: &mut Vec<DspWidget<'a>>) {
        use WWidgetDeclType as W;
        if let Some(decl) = self.widget_decls.pop_front() {
            let label = unsafe { CStr::from_ptr(decl.label) }
                .to_str()
                .unwrap()
                .to_string();
            let mb_widget = match decl.typ {
                W::TAB_BOX => Some(DspWidget::TabGroup {
                    label,
                    inner: vec![],
                    selected: 0,
                }),
                W::HORIZONTAL_BOX | W::VERTICAL_BOX => Some(DspWidget::Box {
                    layout: BoxLayout::from_decl_type(decl.typ),
                    label,
                    inner: vec![],
                }),
                W::CLOSE_BOX => None,
                W::BUTTON | W::CHECK_BUTTON => Some(DspWidget::Button {
                    layout: ButtonLayout::from_decl_type(decl.typ),
                    label,
                    zone: unsafe { decl.zone.as_mut() }.unwrap(),
                }),
                W::HORIZONTAL_SLIDER | W::VERTICAL_SLIDER | W::NUM_ENTRY => {
                    Some(DspWidget::Numeric {
                        layout: NumericLayout::from_decl_type(decl.typ),
                        label,
                        zone: unsafe { decl.zone.as_mut() }.unwrap(),
                        init: decl.init,
                        min: decl.min,
                        max: decl.max,
                        step: decl.step,
                    })
                }
                W::HORIZONTAL_BARGRAPH | W::VERTICAL_BARGRAPH => Some(DspWidget::Bargraph {
                    layout: BargraphLayout::from_decl_type(decl.typ),
                    label,
                    zone: unsafe { decl.zone.as_mut() }.unwrap(),
                    min: decl.min,
                    max: decl.max,
                }),
            };
            if let Some(widget) = mb_widget {
                cur_level.push(widget);
                match cur_level.last_mut().unwrap().inner_mut() {
                    Some(inner) => self.build_widgets(inner),
                    _ => self.build_widgets(cur_level),
                }
            } // If we don't have a widget, it means we had a CLOSE_BOX declaration, so we just go up
        }
    }
}

pub(crate) extern "C" fn widget_decl_callback(builder_ptr: *mut c_void, decl: WWidgetDecl) {
    let builder = unsafe { (builder_ptr as *mut DspWidgetsBuilder).as_mut() }.unwrap();
    builder.widget_decls.push_back(decl);
}
