impl Parser {
    fn parse_structured_sigil(&mut self) -> Option<Expr> {
        if !self.peek_symbol("~") {
            return None;
        }
        let checkpoint = self.pos;
        let start_span = self.peek_span().unwrap_or_else(|| self.previous_span());
        self.pos += 1;
        if self.consume_ident_text("map").is_some() {
            return self.parse_map_literal(start_span);
        }
        if self.consume_ident_text("set").is_some() {
            return self.parse_set_literal(start_span);
        }
        if self.consume_ident_text("mat").is_some() {
            return self.parse_mat_literal(start_span);
        }
        if self.consume_ident_text("path").is_some() {
            return self.parse_path_literal(start_span);
        }
        self.pos = checkpoint;
        None
    }
}

include!("sigils/html.rs");
include!("sigils/gtk.rs");
