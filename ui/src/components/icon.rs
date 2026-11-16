use leptos::prelude::*;

/// The icons the interface draws. Each is one stroked path on a 16×16 grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconName {
    Prev,
    Next,
    Check,
    Open,
    Lines,
    Paperclip,
}

impl IconName {
    fn path(self) -> &'static str {
        match self {
            IconName::Prev => "M10 3.500 5.500 8l4.500 4.500",
            IconName::Next => "M6 3.500 10.500 8 6 12.500",
            IconName::Check => "M3.500 8.300l3 3 6-6.600",
            IconName::Open => "M9 2.500h4.500V7M13.500 2.500 8 8M11.500 9.500v4h-9v-9h4",
            IconName::Lines => "M3 4.500h10M3 8h10M3 11.500h6",
            IconName::Paperclip => {
                "M10.500 5.500 6.200 9.800a1.300 1.300 0 0 0 1.800 1.800l4.600-4.600a2.600 2.600 0 0 0-3.700-3.700L4.300 7.900a3.900 3.900 0 0 0 5.500 5.500l3.200-3.200"
            }
        }
    }
}

#[component]
pub fn Icon(name: IconName, #[prop(default = 16)] size: u32) -> impl IntoView {
    view! {
        <svg
            width=size
            height=size
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            stroke-width="1.4"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
            focusable="false"
        >
            <path d=name.path() />
        </svg>
    }
}
