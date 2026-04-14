# RAII with guards

Description

RAII stands for ”Resource Acquisition is Initialisation” which is a terrible name. The essence of the
pattern is that resource initialisation is done in the constructor of an object and finalisation in the
destructor. This pattern is extended in Rust by using a RAII object as a guard of some resource and
relying on the type system to ensure that access is always mediated by the guard object.

Example

Mutex guards are the classic example of this pattern from the std library (this is a simplified version
of the real implementation):

```rust
use std::ops::Deref;

struct Foo {}

struct Mutex<T> {
    // We keep a reference to our data: T here.
    //..
}

struct MutexGuard<'a, T: 'a> {
    data: &'a T,
    //..
}

// Locking the mutex is explicit.
impl<T> Mutex<T> {
    fn lock(&self) -> MutexGuard<T> {
        // Lock the underlying OS mutex.
        //..

        // MutexGuard keeps a reference to self
        MutexGuard { data: &self }
    }
}

// Destructor for unlocking the mutex.
impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        // Unlock the underlying OS mutex.
        //..
    }
}

// Implementing Deref means we can treat MutexGuard like a pointer to T.
impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.data
    }
}

fn baz(x: Mutex<Foo>) {
    let xx = x.lock();
    xx.foo(); // foo is a method on Foo.
    // The borrow checker ensures we can't store a reference to the
    // underlying Foo which will outlive the guard xx.

    // x is unlocked when we exit this function and xx's destructor is executed.
}
```

Motivation

Where a resource must be finalised after use, RAII can be used to do this finalisation. If it is an error
to access that resource after finalisation, then this pattern can be used to prevent such errors.

Advantages

Prevents errors where a resource is not finalised and where a resource is used after finalisation.

Discussion

RAII is a useful pattern for ensuring resources are properly deallocated or finalised. We can make
use of the borrow checker in Rust to statically prevent errors stemming from using resources after
finalisation takes place.

See also

- Finalisation in destructors idiom
- RAII is a common pattern in C++: cppreference.com, wikipedia.
- Style guide entry (currently just a placeholder).

Last change: 2026-01-03, commit:f279f35
