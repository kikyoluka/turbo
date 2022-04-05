use std::{mem::take, sync::Arc};

use crate::analyzer::FreeVarKind;

use super::{ConstantNumber, ConstantValue, JsValue, ObjectPart};

pub fn replace_builtin(value: &mut JsValue) -> bool {
    match value {
        JsValue::Member(box ref mut obj, ref mut prop) => {
            match obj {
                JsValue::Constant(_) => {
                    value.make_unknown("property on constant");
                    true
                }
                JsValue::Url(_) => {
                    value.make_unknown("property on url");
                    true
                }
                JsValue::Concat(_) => {
                    value.make_unknown("property on string");
                    true
                }
                JsValue::Add(_) => {
                    value.make_unknown("property on number or string");
                    true
                }
                JsValue::Unknown(_, _) => {
                    value.make_unknown("property on unknown");
                    true
                }
                JsValue::Function(_) => {
                    value.make_unknown("property on function");
                    true
                }
                JsValue::Alternatives(alts) => {
                    *value = JsValue::Alternatives(
                        take(alts)
                            .into_iter()
                            .map(|alt| JsValue::Member(box alt, prop.clone()))
                            .collect(),
                    );
                    true
                }
                JsValue::Array(array) => {
                    fn items_to_alternatives(
                        items: &mut Vec<JsValue>,
                        prop: &mut JsValue,
                    ) -> JsValue {
                        items.push(JsValue::Unknown(
                            Some(Arc::new(JsValue::Member(
                                box JsValue::Array(Vec::new()),
                                box take(prop),
                            ))),
                            "unknown array prototype methods or values",
                        ));
                        JsValue::Alternatives(take(items))
                    }
                    match &mut **prop {
                        JsValue::Unknown(_, _) => {
                            *value = items_to_alternatives(array, prop);
                            true
                        }
                        JsValue::Constant(ConstantValue::Num(ConstantNumber(num))) => {
                            let index: usize = *num as usize;
                            if index as f64 == *num && index < array.len() {
                                *value = array.swap_remove(index);
                                true
                            } else {
                                *value = JsValue::Unknown(
                                    Some(Arc::new(JsValue::Member(box take(obj), box take(prop)))),
                                    "invalid index",
                                );
                                true
                            }
                        }
                        JsValue::Constant(_) => {
                            value.make_unknown("non-num constant property on array");
                            true
                        }
                        JsValue::Array(_) => {
                            value.make_unknown("array property on array");
                            true
                        }
                        JsValue::Object(_) => {
                            value.make_unknown("object property on array");
                            true
                        }
                        JsValue::Url(_) => {
                            value.make_unknown("url property on array");
                            true
                        }
                        JsValue::Function(_) => {
                            value.make_unknown("function property on array");
                            true
                        }
                        JsValue::Alternatives(alts) => {
                            *value = JsValue::Alternatives(
                                take(alts)
                                    .into_iter()
                                    .map(|alt| JsValue::Member(box obj.clone(), box alt))
                                    .collect(),
                            );
                            true
                        }
                        JsValue::Concat(_) | JsValue::Add(_) => {
                            if prop.has_placeholder() {
                                // keep the member infact since it might be handled later
                                false
                            } else {
                                *value = items_to_alternatives(array, prop);
                                true
                            }
                        }
                        JsValue::FreeVar(_)
                        | JsValue::Variable(_)
                        | JsValue::Call(_, _)
                        | JsValue::MemberCall(..)
                        | JsValue::Member(_, _)
                        | JsValue::WellKnownObject(_)
                        | JsValue::Argument(_)
                        | JsValue::WellKnownFunction(_)
                        | JsValue::Module(_) => {
                            // keep the member infact since it might be handled later
                            return false;
                        }
                    };
                    true
                }
                JsValue::Object(parts) => {
                    fn parts_to_alternatives(
                        parts: &mut Vec<ObjectPart>,
                        prop: &mut Box<JsValue>,
                    ) -> JsValue {
                        let mut values = Vec::new();
                        for part in parts {
                            match part {
                                ObjectPart::KeyValue(_, value) => {
                                    values.push(take(value));
                                }
                                ObjectPart::Spread(value) => {
                                    values.push(JsValue::Unknown(
                                        Some(Arc::new(JsValue::Member(
                                            box JsValue::Object(vec![take(part)]),
                                            prop.clone(),
                                        ))),
                                        "spreaded object",
                                    ));
                                }
                            }
                        }
                        values.push(JsValue::Unknown(
                            Some(Arc::new(JsValue::Member(
                                box JsValue::Object(Vec::new()),
                                box take(prop),
                            ))),
                            "unknown object prototype methods or values",
                        ));
                        JsValue::Alternatives(values)
                    }
                    match &mut **prop {
                        JsValue::Unknown(_, _) => {
                            *value = parts_to_alternatives(parts, prop);
                            true
                        }
                        JsValue::Constant(_) => {
                            for part in parts.into_iter().rev() {
                                match part {
                                    ObjectPart::KeyValue(key, val) => {
                                        if key == &**prop {
                                            *value = take(val);
                                            return true;
                                        }
                                    }
                                    ObjectPart::Spread(_) => {
                                        value.make_unknown("spreaded object");
                                        return true;
                                    }
                                }
                            }
                            *value = JsValue::FreeVar(FreeVarKind::Other("undefined".into()));
                            true
                        }
                        JsValue::Array(_) => {
                            value.make_unknown("array property on object");
                            true
                        }
                        JsValue::Object(_) => {
                            value.make_unknown("object property on object");
                            true
                        }
                        JsValue::Url(_) => {
                            value.make_unknown("url property on object");
                            true
                        }
                        JsValue::Function(_) => {
                            value.make_unknown("function property on object");
                            true
                        }
                        JsValue::Alternatives(alts) => {
                            *value = JsValue::Alternatives(
                                take(alts)
                                    .into_iter()
                                    .map(|alt| JsValue::Member(box obj.clone(), box alt))
                                    .collect(),
                            );
                            true
                        }
                        JsValue::Concat(_) | JsValue::Add(_) => {
                            if prop.has_placeholder() {
                                // keep the member infact since it might be handled later
                                false
                            } else {
                                *value = parts_to_alternatives(parts, prop);
                                true
                            }
                        }
                        JsValue::FreeVar(_)
                        | JsValue::Variable(_)
                        | JsValue::Call(_, _)
                        | JsValue::MemberCall(..)
                        | JsValue::Member(_, _)
                        | JsValue::WellKnownObject(_)
                        | JsValue::Argument(_)
                        | JsValue::WellKnownFunction(_)
                        | JsValue::Module(_) => {
                            // keep the member infact since it might be handled later
                            debug_assert!(prop.has_placeholder());
                            false
                        }
                    }
                }
                JsValue::FreeVar(_)
                | JsValue::Variable(_)
                | JsValue::Call(_, _)
                | JsValue::MemberCall(..)
                | JsValue::Member(_, _)
                | JsValue::WellKnownObject(_)
                | JsValue::Argument(_)
                | JsValue::WellKnownFunction(_)
                | JsValue::Module(_) => {
                    // keep the member infact since it might be handled later
                    debug_assert!(obj.has_placeholder());
                    false
                }
            }
        }
        JsValue::MemberCall(box ref mut obj, box ref mut prop, ref mut args) => {
            match obj {
                JsValue::Array(items) => match prop {
                    JsValue::Constant(ConstantValue::Str(str)) => match &**str {
                        "concat" => {
                            if args.iter().all(|arg| {
                                matches!(
                                    arg,
                                    JsValue::Array(_)
                                        | JsValue::Constant(_)
                                        | JsValue::Url(_)
                                        | JsValue::Concat(_)
                                        | JsValue::Add(_)
                                        | JsValue::WellKnownObject(_)
                                        | JsValue::WellKnownFunction(_)
                                        | JsValue::Function(_)
                                )
                            }) {
                                for arg in args {
                                    match arg {
                                        JsValue::Array(inner) => {
                                            items.extend(take(inner));
                                        }
                                        JsValue::Constant(_)
                                        | JsValue::Url(_)
                                        | JsValue::Concat(_)
                                        | JsValue::Add(_)
                                        | JsValue::WellKnownObject(_)
                                        | JsValue::WellKnownFunction(_)
                                        | JsValue::Function(_) => {
                                            items.push(take(arg));
                                        }
                                        _ => {
                                            unreachable!();
                                        }
                                    }
                                }
                                *value = take(obj);
                                return true;
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                },
                JsValue::Alternatives(alts) => {
                    *value = JsValue::Alternatives(
                        take(alts)
                            .into_iter()
                            .map(|alt| JsValue::MemberCall(box alt, box prop.clone(), args.clone()))
                            .collect(),
                    );
                    return true;
                }
                _ => {}
            }
            *value = JsValue::Call(
                box JsValue::Member(box take(obj), box take(prop)),
                take(args),
            );
            true
        }
        JsValue::Call(box ref mut callee, ref mut args) => {
            match callee {
                JsValue::Unknown(inner, explainer) => {
                    value.make_unknown("call of unknown function");
                    true
                }
                JsValue::Array(_) => {
                    value.make_unknown("call of array");
                    true
                }
                JsValue::Object(_) => {
                    value.make_unknown("call of object");
                    true
                }
                JsValue::Constant(_) => {
                    value.make_unknown("call of constant");
                    true
                }
                JsValue::Url(_) => {
                    value.make_unknown("call of url");
                    true
                }
                JsValue::Concat(_) => {
                    value.make_unknown("call of string");
                    true
                }
                JsValue::Add(_) => {
                    value.make_unknown("call of number or string");
                    true
                }
                JsValue::Function(box ref mut return_value) => {
                    let mut return_value = take(return_value);
                    return_value.visit_mut_conditional(
                        |value| {
                            if let JsValue::Function(_) = value {
                                false
                            } else {
                                true
                            }
                        },
                        &mut |value| match value {
                            JsValue::Argument(index) => {
                                if let Some(arg) = args.get(*index).cloned() {
                                    *value = arg;
                                } else {
                                    *value =
                                        JsValue::FreeVar(FreeVarKind::Other("undefined".into()))
                                }
                                true
                            }

                            _ => false,
                        },
                    );

                    *value = return_value;
                    true
                }
                JsValue::Alternatives(alts) => {
                    *value = JsValue::Alternatives(
                        take(alts)
                            .into_iter()
                            .map(|alt| JsValue::Call(box alt, args.clone()))
                            .collect(),
                    );
                    true
                }
                JsValue::FreeVar(_)
                | JsValue::Variable(_)
                | JsValue::Call(_, _)
                | JsValue::MemberCall(..)
                | JsValue::Member(_, _)
                | JsValue::WellKnownObject(_)
                | JsValue::Argument(_)
                | JsValue::WellKnownFunction(_)
                | JsValue::Module(_) => {
                    // keep the call infact since it might be handled later
                    debug_assert!(callee.has_placeholder());
                    false
                }
            }
        }
        JsValue::Object(parts) => {
            if parts
                .iter()
                .any(|part| matches!(part, ObjectPart::Spread(JsValue::Object(_))))
            {
                let old_parts = take(parts);
                for part in old_parts {
                    if let ObjectPart::Spread(JsValue::Object(inner_parts)) = part {
                        parts.extend(inner_parts);
                    } else {
                        parts.push(part);
                    }
                }
                true
            } else {
                false
            }
        }
        _ => false,
    }
}
