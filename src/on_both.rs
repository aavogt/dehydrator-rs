use sht31::error::Result as R;

pub struct OnBoth<T> (pub T,pub T);

// I tried returning a closure, but it's not polymorphic https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=721b56bc09f8b9d72201f27df85ccf75
// this adapts xiretza from #rust:matrix.org's solution <https://play.rust-lang.org/?version=nightly&mode=debug&edition=2021&gist=83608f93fd1a96fbded955618246ba02>
// TODO parametrize on R
impl<T, F, A> FnOnce<(F,)> for OnBoth<T>
where
    F: FnMut(&mut T) -> R<A>,
{
    type Output = R<(A, A)>;

    extern "rust-call" fn call_once(mut self, args: (F,)) -> R<(A, A)> {
        self.call_mut(args)
    }
}

impl<T, F, A> FnMut<(F,)> for OnBoth<T>
where
    F: FnMut(&mut T) -> R<A>,
{
    extern "rust-call" fn call_mut(&mut self, (mut f,): (F,)) -> R<(A, A)> {
        let a = f(&mut self.0)?;
        let b = f(&mut self.1)?;
        Ok((a, b))
    }
}

