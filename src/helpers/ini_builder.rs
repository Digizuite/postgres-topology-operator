use std::fmt::Display;

pub struct IniBuilder {
    output: String,
}

impl IniBuilder {
    pub fn add_section(&mut self, section: &str) {
        self.output.push_str(&format!("[{}]\n", section));
    }

    pub fn add_setting(&mut self, key: &str, value: impl Display) {
        self.output.push_str(&format!("{} = {}\n", key, value));
    }

    pub fn add_optional(&mut self, key: &str, value: &Option<impl Display>) {
        if let Some(value) = value {
            self.add_setting(key, value);
        }
    }

    pub fn add_comma_separated(&mut self, key: &str, values: &Vec<impl Display>) {
        if values.is_empty() {
            return;
        }


        let s = values.iter()
            .enumerate()
            .fold(String::new(), |mut s, (index, value)| {
                if index > 0 {
                    s.push_str(", ");
                }
                s.push_str(&format!("{}", value));
                s
            });

        self.add_setting(key, s);
    }

    pub fn build(self) -> String {
        self.output
    }
}

pub fn new() -> IniBuilder {
    IniBuilder {
        output: String::new(),
    }
}


#[cfg(test)]
mod tests {
    struct TestWrapper {
        pub optional_value: Option<String>,
    }

    #[test]
    fn test_ini_builder() {
        let mut builder = super::new();

        builder.add_section("pgbouncer");
        builder.add_setting("pool_mode", "transaction");
        builder.add_setting("listen_port", 5432);
        builder.add_optional("is_some", &Some("some"));
        builder.add_optional("is_none", &Option::<&str>::None);

        let w = TestWrapper {
            optional_value: Some("some".to_string()),
        };

        builder.add_optional("another_some", &w.optional_value);

        let vec_values = vec!["one", "two", "three"];

        builder.add_comma_separated("comma_separated", &vec_values);

        assert_eq!(builder.output, "[pgbouncer]\npool_mode = transaction\nlisten_port = 5432\nis_some = some\nanother_some = some\ncomma_separated = one, two, three\n");
    }
}
