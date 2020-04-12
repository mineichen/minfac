# Dynamic Dependency Injection
## Why references only?
It was a mayor goal from the beginning on to support references. The idea of having mutable references was dropped very early in the design process because of the difficulty to implement them, if it was possible at all. The third category, owned values, seemed easy to implement and needed to have Arc's for sharing Objects between different threads. However, the service-resolution was quite painful to implement and lead to very verbose code if no macros are used, because one had to specify wether a value or a reference was requested<sup>1</sup>. In my opinion, macros should be avoided if not absolutely necessary because they are always a blackbox the user has difficulties to reason about.

Using references has a very nice property compared to Rc/Arc: No reference could ever outlive a scope, which is made sure during compile-time. This makes it easier, not to accidentally keep references from an older scope. It turned out, that we actually don't need Arc's to share objects among multiple threads.

<sup>1</sup> References can't be handled as normal values because they don't implement Any, which is used to identify a type