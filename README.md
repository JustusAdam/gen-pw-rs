# A strong password generator

This is essentially [this
gist](https://gist.github.com/JustusAdam/e006e77a8407cc76e6dc853f2a255ff2) that
I published a while back but implemented in Rust, because what I really need in
my strong password generator is #performance.

What this application actually does is it generates strong passwords (e.g.
random) that satisfy a configurable set of conditions either by picking letters
from the range `a-z` with the `chars` subcommand or by sampling a dictionary
with the `dict` subcommand.

The set of supported constraints the resulting password must adhere to, which
can be turned off (with `--exclude`) or selectively turned on (with `--require`)
is:

Include at least one

- `Number`: i.e. character in `0-9`
- `Symbol`: the list of allowed symbol is configurable with `--symbols`
- `LowerCaseLetter`
- `UpperCaseLetter`

If none of the options are specified all constraints are enabled.

In addition there is always a minimum length constraint (`--min`) with a default
value of `10` and a maximum length constraint (`--max`) with a default value of
`20`.

As an implementation detail the tool is implemented by randomly (though not
stupidly) creating candidates and then testing the constraints on them. The
application will test at most `1000` candidates before giving up and reporting
an error. This limit can be configured with `--tries`. When passed `--debug` the
application will report the reason for rejecting each candidate.

All command line options can also be passed as environment variables with their
names converted to `SCREAMING_SNAKE_CASE`.