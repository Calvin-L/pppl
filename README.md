# Persistent Parallel Programming Language (PPPL)

PPPL is pronounced "pipple".  It's a programming system with some bold new
ideas.

When I made the project I accidentally typed an extra "p", so for historical
reasons the executable is named `ppppl`.  I guess we could backronym that to
"Perfect Persistent Parallel Programming Language"?


## Concepts

You are likely reading this on a computer.  That's not new.

Your computer has something called _pmemory_ (pronounced "memory"; think
"pneumatic").  If you have never run a PPPL program, your computer still has
pmemory; it just happens to be empty right now.

Pmemory has two parts.  One is a program, and the other is a single value.
Programs are described below.

Pmemory is persistent.  It will be there when you restart your computer, and
all the actions your program takes are permanent.


### What you, a human, can do

 - Overwrite your computer's pmemory's program (see `pppl load` below)
 - Let the computer make progresss running your program (see `pppl run` below)
 - Read and write your computer's pmemory's value (see `pppl read` and
   `pppl write` below)

Obvously, you can do many other things as well, such as "write an email" or
"make lunch".  The list above is not meant to be restrictive; it is just to
inform you of some options you might not have known you have.


### What PPPL can do

A PPPL program is a list of blocks.  For example, here's a single block that
counts up to 100:

```
def up:
    require x < 100;
    x := x + 1;
```

Variable names are shorthand for those keys in the single pmemory value, which
is named `.`.  So: `x == .["x"]`.

Each block can have requirements like `x < 100`.  The block will not run if its
requirements are not satisfied.  It also won't run if it would produce an
exception (for instance, if `x` is not an integer).

The effects of a block happen atomically.  For instance, here's a block that
swaps `x` and `y`:

```
def swap:
    x := y;
    y := x;
```

PPPL works like this:

```python
while True:
    atomically:
        pick a block whose requirements are satisfied
        execute the side-effects of that block
```

The current implementation is single-threaded.  To achieve true parallelism,
run multiple copies of `ppppl run` at once (see commands below).


#### Data Parallelism

Sometimes doing two different things in parallel isn't enough.  Sometimes you
need to do the same thing in parallel with many pieces of data.

```
def inc(x in m):
    m[x] := m[x] + 1;
```

Blocks can take parameters, such as `x in m` above.  If `m` is a dictionary,
then `inc` can be run with `x` set to any key of `m`.


#### Doing Things in Sequence

In some programming langauges, you have to do a lot of work to spawn threads
and exploit multiple cores.  In PPPL, parallelism is the default!  ...But
you'll have to do a lot of work to force sequential execution if you need it.

```
def step1:
    require pc = 0;
    pc := pc + 1;

def step2:
    require pc = 1;
    pc := pc + 1;

def step3:
    require pc = 2;
    pc := pc + 1;
```


#### Expression Summary

```
Booleans

    true, false,
    ==, !=,
    &&, ||, !,
    if _ then _ else _

Numbers

    +, -, *, /, %,
    >, <, >=, <=

Dictionaries

    {}          (the empty map)
    x in m      (test key presence)
    m[x]        (lookup value by key)
```


## Concrete Instructions

First, compile this project:

    cargo build --release

Then load a program:

    cat examples/div3.pppl
    ./target/release/ppppl load examples/div3.pppl

Then run it:

    ./target/release/ppppl run

It's always safe to have multiple `run` commands going at once.  In fact,
that's how to achieve parallelism with the current implementation.

While it's running, you might want to interact with it a bit:

    ./target/release/ppppl write 'x := 1000;'
    ./target/release/ppppl read done
    ./target/release/ppppl read q
    ./target/release/ppppl read r

`examples/div3.pppl` computes division by 3 very slowly.  When it finishes, it
writes `done := true;` and the result of `x/3` is in `q` and `r`.

You can "restart" the program by writing a new value to `x`---although, this
is something that is implemented in the `begin` block, and is not part of PPPL
by default.
