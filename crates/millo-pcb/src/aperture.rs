use std::collections::BTreeMap;

use clipper2::{EndType, JoinType, Path, Paths, Point};
use lib_gerber_edit::gerber_types::{
    Aperture, MacroBoolean, MacroContent, MacroDecimal, MacroInteger,
};

use crate::{
    PcbError, PcbPoint,
    geometry::{CamPath, CamPaths, circle, combine, polygon, rectangle},
};

pub(crate) fn flash(
    aperture: &Aperture,
    macros: &std::collections::HashMap<String, Vec<MacroContent>>,
    center: PcbPoint,
    source_name: &str,
) -> Result<CamPaths, PcbError> {
    let (outer, hole) = match aperture {
        Aperture::Circle(value) => (
            Paths::new(vec![circle(center, value.diameter / 2.0)]),
            value.hole_diameter,
        ),
        Aperture::Rectangle(value) => (
            Paths::new(vec![rectangle(center, value.x, value.y)]),
            value.hole_diameter,
        ),
        Aperture::Obround(value) => (obround(center, value.x, value.y), value.hole_diameter),
        Aperture::Polygon(value) => (
            Paths::new(vec![polygon(
                center,
                value.diameter / 2.0,
                usize::from(value.vertices),
                value.rotation.unwrap_or(0.0).to_radians(),
            )]),
            value.hole_diameter,
        ),
        Aperture::Macro(name, modifiers) => {
            return render_macro(
                name,
                modifiers.as_deref().unwrap_or_default(),
                macros,
                center,
                source_name,
            );
        }
    };
    match hole {
        Some(diameter) if diameter > 0.0 => combine(
            outer,
            Paths::new(vec![circle(center, diameter / 2.0)]),
            false,
        ),
        _ => Ok(outer),
    }
}

pub(crate) fn stroke(
    aperture: &Aperture,
    centerline: CamPath,
    source_name: &str,
) -> Result<CamPaths, PcbError> {
    let paths = match aperture {
        Aperture::Circle(value) => {
            centerline.inflate(value.diameter / 2.0, JoinType::Round, EndType::Round, 2.0)
        }
        Aperture::Rectangle(value) => {
            centerline.minkowski_sum(rectangle(PcbPoint::default(), value.x, value.y), false)
        }
        Aperture::Obround(value) => {
            let kernel = obround(PcbPoint::default(), value.x, value.y);
            let kernel = kernel
                .first()
                .ok_or_else(|| PcbError::EmptyLayer(source_name.to_owned()))?;
            centerline.minkowski_sum(kernel.clone(), false)
        }
        Aperture::Polygon(value) => centerline.minkowski_sum(
            polygon(
                PcbPoint::default(),
                value.diameter / 2.0,
                usize::from(value.vertices),
                value.rotation.unwrap_or(0.0).to_radians(),
            ),
            false,
        ),
        Aperture::Macro(name, _) => {
            return Err(unsupported(
                source_name,
                format!("macro aperture {name} used for a draw"),
            ));
        }
    };
    Ok(paths.simplify(0.002, false))
}

fn render_macro(
    name: &str,
    modifiers: &[MacroDecimal],
    macros: &std::collections::HashMap<String, Vec<MacroContent>>,
    center: PcbPoint,
    source_name: &str,
) -> Result<CamPaths, PcbError> {
    let content = macros
        .get(name)
        .ok_or_else(|| unsupported(source_name, format!("undefined aperture macro {name}")))?;
    let mut variables = BTreeMap::new();
    for (index, modifier) in modifiers.iter().enumerate() {
        variables.insert(
            (index + 1) as u32,
            decimal(modifier, &variables, source_name)?,
        );
    }
    let mut image = CamPaths::default();
    for item in content {
        match item {
            MacroContent::VariableDefinition(definition) => {
                let value = expression(&definition.expression, &variables, source_name)?;
                variables.insert(definition.number, value);
            }
            MacroContent::Comment(_) => {}
            MacroContent::Circle(value) => {
                let angle = value
                    .angle
                    .as_ref()
                    .map(|value| decimal(value, &variables, source_name))
                    .transpose()?
                    .unwrap_or(0.0)
                    .to_radians();
                let local = rotate_point(
                    PcbPoint {
                        x_mm: decimal(&value.center.0, &variables, source_name)?,
                        y_mm: decimal(&value.center.1, &variables, source_name)?,
                    },
                    angle,
                );
                let primitive = Paths::new(vec![circle(
                    translate(local, center),
                    decimal(&value.diameter, &variables, source_name)? / 2.0,
                )]);
                image = combine(
                    image,
                    primitive,
                    boolean(&value.exposure, &variables, source_name)?,
                )?;
            }
            MacroContent::VectorLine(value) => {
                let angle = decimal(&value.angle, &variables, source_name)?.to_radians();
                let start = translate(
                    rotate_point(pair(&value.start, &variables, source_name)?, angle),
                    center,
                );
                let end = translate(
                    rotate_point(pair(&value.end, &variables, source_name)?, angle),
                    center,
                );
                let centerline: CamPath =
                    vec![(start.x_mm, start.y_mm), (end.x_mm, end.y_mm)].into();
                let primitive = centerline.inflate(
                    decimal(&value.width, &variables, source_name)? / 2.0,
                    JoinType::Round,
                    EndType::Square,
                    2.0,
                );
                image = combine(
                    image,
                    primitive,
                    boolean(&value.exposure, &variables, source_name)?,
                )?;
            }
            MacroContent::CenterLine(value) => {
                let local_center = pair(&value.center, &variables, source_name)?;
                let width = decimal(&value.dimensions.0, &variables, source_name)?;
                let height = decimal(&value.dimensions.1, &variables, source_name)?;
                let primitive = transform_paths(
                    Paths::new(vec![rectangle(local_center, width, height)]),
                    decimal(&value.angle, &variables, source_name)?.to_radians(),
                    center,
                );
                image = combine(
                    image,
                    primitive,
                    boolean(&value.exposure, &variables, source_name)?,
                )?;
            }
            MacroContent::Outline(value) => {
                let path = Path::new(
                    value
                        .points
                        .iter()
                        .map(|point| pair(point, &variables, source_name))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .map(|point| Point::new(point.x_mm, point.y_mm))
                        .collect(),
                );
                let primitive = transform_paths(
                    Paths::new(vec![path]),
                    decimal(&value.angle, &variables, source_name)?.to_radians(),
                    center,
                );
                image = combine(
                    image,
                    primitive,
                    boolean(&value.exposure, &variables, source_name)?,
                )?;
            }
            MacroContent::Polygon(value) => {
                let vertices = integer(&value.vertices, &variables, source_name)? as usize;
                let local_center = pair(&value.center, &variables, source_name)?;
                let primitive = transform_paths(
                    Paths::new(vec![polygon(
                        local_center,
                        decimal(&value.diameter, &variables, source_name)? / 2.0,
                        vertices,
                        0.0,
                    )]),
                    decimal(&value.angle, &variables, source_name)?.to_radians(),
                    center,
                );
                image = combine(
                    image,
                    primitive,
                    boolean(&value.exposure, &variables, source_name)?,
                )?;
            }
            MacroContent::Moire(value) => {
                let angle = decimal(&value.angle, &variables, source_name)?.to_radians();
                let local_center = pair(&value.center, &variables, source_name)?;
                let diameter = decimal(&value.diameter, &variables, source_name)?;
                let thickness = decimal(&value.ring_thickness, &variables, source_name)?;
                let gap = decimal(&value.gap, &variables, source_name)?;
                let mut primitive = CamPaths::default();
                for index in 0..value.max_rings {
                    let outer = diameter - 2.0 * f64::from(index) * (thickness + gap);
                    if outer <= 0.0 || thickness <= 0.0 {
                        break;
                    }
                    primitive = combine(
                        primitive,
                        Paths::new(vec![circle(local_center, outer / 2.0)]),
                        true,
                    )?;
                    let inner = outer - 2.0 * thickness;
                    if inner > 0.0 {
                        primitive = combine(
                            primitive,
                            Paths::new(vec![circle(local_center, inner / 2.0)]),
                            false,
                        )?;
                    }
                }
                let cross_width = decimal(&value.cross_hair_thickness, &variables, source_name)?;
                let cross_length = decimal(&value.cross_hair_length, &variables, source_name)?;
                primitive = combine(
                    primitive,
                    Paths::new(vec![
                        rectangle(local_center, cross_length, cross_width),
                        rectangle(local_center, cross_width, cross_length),
                    ]),
                    true,
                )?;
                image = combine(image, transform_paths(primitive, angle, center), true)?;
            }
            MacroContent::Thermal(value) => {
                let local_center = pair(&value.center, &variables, source_name)?;
                let outer = decimal(&value.outer_diameter, &variables, source_name)?;
                let inner = decimal(&value.inner_diameter, &variables, source_name)?;
                let gap = decimal(&value.gap, &variables, source_name)?;
                let mut primitive = Paths::new(vec![circle(local_center, outer / 2.0)]);
                if inner > 0.0 {
                    primitive = combine(
                        primitive,
                        Paths::new(vec![circle(local_center, inner / 2.0)]),
                        false,
                    )?;
                }
                primitive = combine(
                    primitive,
                    Paths::new(vec![
                        rectangle(local_center, outer * 1.1, gap),
                        rectangle(local_center, gap, outer * 1.1),
                    ]),
                    false,
                )?;
                image = combine(
                    image,
                    transform_paths(
                        primitive,
                        decimal(&value.angle, &variables, source_name)?.to_radians(),
                        center,
                    ),
                    true,
                )?;
            }
        }
    }
    if image.is_empty() {
        Err(unsupported(
            source_name,
            format!("aperture macro {name} produced no geometry"),
        ))
    } else {
        Ok(image.simplify(0.002, false))
    }
}

fn obround(center: PcbPoint, width: f64, height: f64) -> CamPaths {
    if width >= height {
        let half = (width - height) / 2.0;
        let line: CamPath = vec![
            (center.x_mm - half, center.y_mm),
            (center.x_mm + half, center.y_mm),
        ]
        .into();
        line.inflate(height / 2.0, JoinType::Round, EndType::Round, 2.0)
    } else {
        let half = (height - width) / 2.0;
        let line: CamPath = vec![
            (center.x_mm, center.y_mm - half),
            (center.x_mm, center.y_mm + half),
        ]
        .into();
        line.inflate(width / 2.0, JoinType::Round, EndType::Round, 2.0)
    }
}

fn transform_paths(paths: CamPaths, angle: f64, translation: PcbPoint) -> CamPaths {
    Paths::new(
        paths
            .iter()
            .map(|path| {
                Path::new(
                    path.iter()
                        .map(|point| {
                            let point = translate(
                                rotate_point(
                                    PcbPoint {
                                        x_mm: point.x(),
                                        y_mm: point.y(),
                                    },
                                    angle,
                                ),
                                translation,
                            );
                            Point::new(point.x_mm, point.y_mm)
                        })
                        .collect(),
                )
            })
            .collect(),
    )
}

fn rotate_point(point: PcbPoint, angle: f64) -> PcbPoint {
    let (sin, cos) = angle.sin_cos();
    PcbPoint {
        x_mm: point.x_mm * cos - point.y_mm * sin,
        y_mm: point.x_mm * sin + point.y_mm * cos,
    }
}

fn translate(point: PcbPoint, translation: PcbPoint) -> PcbPoint {
    PcbPoint {
        x_mm: point.x_mm + translation.x_mm,
        y_mm: point.y_mm + translation.y_mm,
    }
}

fn pair(
    value: &(MacroDecimal, MacroDecimal),
    variables: &BTreeMap<u32, f64>,
    source_name: &str,
) -> Result<PcbPoint, PcbError> {
    Ok(PcbPoint {
        x_mm: decimal(&value.0, variables, source_name)?,
        y_mm: decimal(&value.1, variables, source_name)?,
    })
}

fn decimal(
    value: &MacroDecimal,
    variables: &BTreeMap<u32, f64>,
    source_name: &str,
) -> Result<f64, PcbError> {
    match value {
        MacroDecimal::Value(value) => Ok(*value),
        MacroDecimal::Variable(number) => variable(*number, variables, source_name),
        MacroDecimal::Expression(value) => expression(value, variables, source_name),
    }
}

fn boolean(
    value: &MacroBoolean,
    variables: &BTreeMap<u32, f64>,
    source_name: &str,
) -> Result<bool, PcbError> {
    let value = match value {
        MacroBoolean::Value(value) => return Ok(*value),
        MacroBoolean::Variable(number) => variable(*number, variables, source_name)?,
        MacroBoolean::Expression(value) => expression(value, variables, source_name)?,
    };
    Ok(value.abs() > f64::EPSILON)
}

fn integer(
    value: &MacroInteger,
    variables: &BTreeMap<u32, f64>,
    source_name: &str,
) -> Result<u32, PcbError> {
    let value = match value {
        MacroInteger::Value(value) => return Ok(*value),
        MacroInteger::Variable(number) => variable(*number, variables, source_name)?,
        MacroInteger::Expression(value) => expression(value, variables, source_name)?,
    };
    if !value.is_finite() || value < 0.0 || value.fract().abs() > 1e-9 {
        return Err(unsupported(
            source_name,
            "non-integer aperture macro vertex count",
        ));
    }
    Ok(value as u32)
}

fn variable(
    number: u32,
    variables: &BTreeMap<u32, f64>,
    source_name: &str,
) -> Result<f64, PcbError> {
    variables.get(&number).copied().ok_or_else(|| {
        unsupported(
            source_name,
            format!("undefined aperture macro variable ${number}"),
        )
    })
}

fn expression(
    source: &str,
    variables: &BTreeMap<u32, f64>,
    source_name: &str,
) -> Result<f64, PcbError> {
    let mut parser = ExpressionParser {
        source: source.as_bytes(),
        cursor: 0,
        variables,
    };
    let value = parser.parse_sum().map_err(|detail| {
        unsupported(
            source_name,
            format!("invalid aperture macro expression `{source}`: {detail}"),
        )
    })?;
    parser.skip_spaces();
    if parser.cursor != parser.source.len() || !value.is_finite() {
        return Err(unsupported(
            source_name,
            format!("invalid aperture macro expression `{source}`"),
        ));
    }
    Ok(value)
}

struct ExpressionParser<'a> {
    source: &'a [u8],
    cursor: usize,
    variables: &'a BTreeMap<u32, f64>,
}

impl ExpressionParser<'_> {
    fn parse_sum(&mut self) -> Result<f64, &'static str> {
        let mut value = self.parse_product()?;
        loop {
            self.skip_spaces();
            match self.peek() {
                Some(b'+') => {
                    self.cursor += 1;
                    value += self.parse_product()?;
                }
                Some(b'-') => {
                    self.cursor += 1;
                    value -= self.parse_product()?;
                }
                _ => return Ok(value),
            }
        }
    }

    fn parse_product(&mut self) -> Result<f64, &'static str> {
        let mut value = self.parse_unary()?;
        loop {
            self.skip_spaces();
            match self.peek() {
                Some(b'x' | b'X') => {
                    self.cursor += 1;
                    value *= self.parse_unary()?;
                }
                Some(b'/') => {
                    self.cursor += 1;
                    let divisor = self.parse_unary()?;
                    if divisor.abs() <= f64::EPSILON {
                        return Err("division by zero");
                    }
                    value /= divisor;
                }
                _ => return Ok(value),
            }
        }
    }

    fn parse_unary(&mut self) -> Result<f64, &'static str> {
        self.skip_spaces();
        match self.peek() {
            Some(b'+') => {
                self.cursor += 1;
                self.parse_unary()
            }
            Some(b'-') => {
                self.cursor += 1;
                Ok(-self.parse_unary()?)
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<f64, &'static str> {
        self.skip_spaces();
        if self.peek() == Some(b'(') {
            self.cursor += 1;
            let value = self.parse_sum()?;
            self.skip_spaces();
            if self.peek() != Some(b')') {
                return Err("missing closing parenthesis");
            }
            self.cursor += 1;
            return Ok(value);
        }
        if self.peek() == Some(b'$') {
            self.cursor += 1;
            let number = self.parse_digits()?;
            return self
                .variables
                .get(&number)
                .copied()
                .ok_or("undefined variable");
        }
        let start = self.cursor;
        while matches!(self.peek(), Some(b'0'..=b'9' | b'.')) {
            self.cursor += 1;
        }
        if start == self.cursor {
            return Err("expected a number");
        }
        std::str::from_utf8(&self.source[start..self.cursor])
            .ok()
            .and_then(|value| value.parse().ok())
            .ok_or("invalid number")
    }

    fn parse_digits(&mut self) -> Result<u32, &'static str> {
        let start = self.cursor;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.cursor += 1;
        }
        if start == self.cursor {
            return Err("expected a variable number");
        }
        std::str::from_utf8(&self.source[start..self.cursor])
            .ok()
            .and_then(|value| value.parse().ok())
            .ok_or("invalid variable number")
    }

    fn skip_spaces(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.cursor += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.cursor).copied()
    }
}

fn unsupported(source_name: &str, feature: impl Into<String>) -> PcbError {
    PcbError::UnsupportedGerberFeature(source_name.to_owned(), feature.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_gerber_macro_precedence_variables_and_parentheses() {
        let variables = BTreeMap::from([(1, 2.0), (2, 3.0)]);
        assert_eq!(expression("$1+$2x4", &variables, "fixture").unwrap(), 14.0);
        assert_eq!(
            expression("($1+$2)x4", &variables, "fixture").unwrap(),
            20.0
        );
        assert_eq!(expression("-$1/2", &variables, "fixture").unwrap(), -1.0);
    }
}
