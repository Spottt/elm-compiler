//! Source AST. IDs point into flat arenas, so destroying a long syntax tree does
//! not recurse and names/literals borrow the source instead of being copied.
use crate::lexer::Kind;
macro_rules! id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u32);
    };
}
id!(ExprId);
id!(PatternId);
id!(TypeId);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}
#[derive(Debug)]
pub struct Node<T> {
    pub span: Span,
    pub kind: T,
}
#[derive(Debug)]
pub enum Expr<'s> {
    Literal(Kind, &'s str),
    Var(&'s str),
    Operator(&'s str),
    Unit,
    List(Vec<ExprId>),
    Tuple(Vec<ExprId>),
    Negate(ExprId),
    Call(ExprId, Vec<ExprId>),
    Binops(ExprId, Vec<(&'s str, ExprId)>),
    Binary(&'s str, ExprId, ExprId),
    Lambda(Vec<PatternId>, ExprId),
    If(ExprId, ExprId, ExprId),
    Let(Vec<Declaration<'s>>, ExprId),
    Case(ExprId, Vec<(PatternId, ExprId)>),
    Accessor(&'s str),
    Access(ExprId, &'s str),
    Record {
        base: Option<&'s str>,
        fields: Vec<(&'s str, ExprId)>,
    },
}
#[derive(Debug)]
pub enum Pattern<'s> {
    Wildcard,
    Var(&'s str),
    Literal(Kind, &'s str),
    Unit,
    Constructor(&'s str, Vec<PatternId>),
    Tuple(Vec<PatternId>),
    List(Vec<PatternId>),
    Record(Vec<&'s str>),
    Cons(PatternId, PatternId),
    Alias(PatternId, &'s str),
}
#[derive(Debug)]
pub enum Type<'s> {
    Var(&'s str),
    Constructor(&'s str, Vec<TypeId>),
    Unit,
    Function(TypeId, TypeId),
    Tuple(Vec<TypeId>),
    Record {
        extension: Option<&'s str>,
        fields: Vec<(&'s str, TypeId)>,
    },
}
#[derive(Debug)]
pub enum Declaration<'s> {
    Annotation {
        name: &'s str,
        ty: TypeId,
    },
    Value {
        name: &'s str,
        arguments: Vec<PatternId>,
        body: ExprId,
    },
    Destruct {
        pattern: PatternId,
        body: ExprId,
    },
    Alias {
        name: &'s str,
        parameters: Vec<&'s str>,
        ty: TypeId,
    },
    Union {
        name: &'s str,
        parameters: Vec<&'s str>,
        variants: Vec<(&'s str, Vec<TypeId>)>,
    },
    Port {
        name: &'s str,
        ty: TypeId,
    },
    Infix {
        associativity: &'s str,
        precedence: u8,
        operator: &'s str,
        function: &'s str,
    },
}
#[derive(Debug)]
pub struct Syntax<'s> {
    pub source: &'s str,
    pub header: crate::module::Header,
    pub declarations: Vec<Declaration<'s>>,
    pub expressions: Vec<Node<Expr<'s>>>,
    pub patterns: Vec<Node<Pattern<'s>>>,
    pub types: Vec<Node<Type<'s>>>,
}
