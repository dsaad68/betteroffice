//! Parses `ppt/tableStyles.xml`.

use ooxml_drawingml::{
    TableCellBorder, TableCellBorders, TableCellStyle, TableStyle, TableStyleList, TableStylePart,
    TableTextStyle,
};

use crate::drawing::{parse_color_container, parse_fill_element, parse_outline_element};
use crate::xml::XmlElement;

pub(crate) fn parse_table_styles(root: &XmlElement) -> TableStyleList {
    TableStyleList {
        default_style_id: root.attribute("def").map(str::to_owned),
        styles: root
            .children_named("tblStyle")
            .map(parse_table_style)
            .collect(),
    }
}

fn parse_table_style(element: &XmlElement) -> TableStyle {
    TableStyle {
        style_id: element.attribute("styleId").unwrap_or_default().to_owned(),
        style_name: element.attribute("styleName").map(str::to_owned),
        whole_table: element.child("wholeTbl").map(parse_part),
        band1_row: element.child("band1H").map(parse_part),
        band2_row: element.child("band2H").map(parse_part),
        first_row: element.child("firstRow").map(parse_part),
        last_row: element.child("lastRow").map(parse_part),
        first_column: element.child("firstCol").map(parse_part),
        last_column: element.child("lastCol").map(parse_part),
    }
}

fn parse_part(element: &XmlElement) -> TableStylePart {
    TableStylePart {
        text: element
            .child("tcTxStyle")
            .map(parse_text_style)
            .unwrap_or_default(),
        cell: element
            .child("tcStyle")
            .map(parse_cell_style)
            .unwrap_or_default(),
    }
}

fn parse_text_style(element: &XmlElement) -> TableTextStyle {
    TableTextStyle {
        bold: on_off(element.attribute("b")),
        italic: on_off(element.attribute("i")),
        color: parse_color_container(element),
    }
}

fn parse_cell_style(element: &XmlElement) -> TableCellStyle {
    TableCellStyle {
        fill: element
            .child("fill")
            .and_then(|fill| fill.child_elements().find_map(parse_fill_element)),
        borders: element
            .child("tcBdr")
            .map(parse_borders)
            .unwrap_or_default(),
    }
}

fn parse_borders(element: &XmlElement) -> TableCellBorders {
    TableCellBorders {
        left: element.child("left").and_then(parse_border),
        right: element.child("right").and_then(parse_border),
        top: element.child("top").and_then(parse_border),
        bottom: element.child("bottom").and_then(parse_border),
        inside_horizontal: element.child("insideH").and_then(parse_border),
        inside_vertical: element.child("insideV").and_then(parse_border),
    }
}

fn parse_border(element: &XmlElement) -> Option<TableCellBorder> {
    let line = element.child("ln")?;
    if line.child("noFill").is_some() {
        return Some(TableCellBorder::None);
    }
    parse_outline_element(line).map(|outline| TableCellBorder::Line(Box::new(outline)))
}

fn on_off(value: Option<&str>) -> Option<bool> {
    match value? {
        "on" | "1" | "true" => Some(true),
        "off" | "0" | "false" => Some(false),
        _ => None,
    }
}
