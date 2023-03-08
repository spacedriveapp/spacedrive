use std::future::Future;

// A trait which allows me to represent a closure, it's future return type and the futures return type with only a single generic.
pub trait AsyncFn
where
    Self: Fn() -> Self::Future + Send + Sync + 'static,
{
    type Output;
    type Future: Future<Output = <Self as AsyncFn>::Output> + Send;
}

impl<TOutput, TFut, TFunc> AsyncFn for TFunc
where
    TFut: Future<Output = TOutput> + Send,
    TFunc: Fn() -> TFut + Send + Sync + 'static,
{
    type Output = TOutput;
    type Future = TFut;
}
