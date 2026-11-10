## Purpose
Shows what each turn of the agent cost, how it ended and what the person did next, so that the person can see which turns end in waiting, which get stopped, and whether a well-specified long run costs less per change than step-by-step work.

## ADDED Requirements

### Requirement: Requests of a turn
Each turn of work-rhythm SHALL own the requests of its main conversation from its start through its end, and the requests of the session's subagents sent in that time. The turn's dollars are theirs, priced as cost-attribution prices them, and its calls are their tool calls. A turn's edit calls are its calls whose class is `editing` (see cost-attribution, Activity of a request). A subagent request sent between turns, while the agent stands still, belongs to no turn.

#### Scenario: A subagent inside a turn
- **WHEN** a turn runs from 10:00 to 10:20 and a subagent of the session sends requests from 10:05 to 10:12
- **THEN** the turn's dollars include those of the subagent's requests

#### Scenario: All turns
- **WHEN** the turns of the sessions the rhythm keeps are summed
- **THEN** their dollars and those of the subagent requests between turns equal the spend of those sessions inside the window

### Requirement: How a turn ends
Each turn SHALL have one end kind, the first that applies:
- `stopped`: the person interrupted it;
- `failed`: its last call failed, was refused or denied;
- `asked`: its last call is a question, or its last response asks (see A response that asks);
- `waiting`: the next hand-off in its session is a wake;
- `committed`: a call of it made a commit, as share-export counts them;
- `changed`: it made an edit call;
- `answered`: none of these.

#### Scenario: Files changed, no commit
- **WHEN** a turn makes 7 edit calls, its last call works, and the response that ended it reads `Done. The tests pass.`
- **THEN** its end kind is `changed`

#### Scenario: A question in the text
- **WHEN** a turn makes 2 edit calls and the response that ended it ends with `Should I also update the README?**`
- **THEN** its end kind is `asked`

#### Scenario: Waiting for its own work
- **WHEN** a turn starts the tests in the background, ends with `Тесты идут, жду результат.`, and the next hand-off is the tests' wake
- **THEN** its end kind is `waiting`

#### Scenario: Stopped while editing
- **WHEN** the person interrupts a turn that made a commit
- **THEN** its end kind is `stopped`

### Requirement: A response that asks
A response SHALL ask the person when its last paragraph, the last block of text after a blank line, matches one of these rules:
- one of its last three sentences holds `?` outside a link;
- it holds words of permission (see Words that ask);
- one of its last three sentences holds a request word;
- one of its last two sentences holds an offer.

A trailing code fence or `Co-Authored-By:` line is skipped. Only the source sees the text: it passes on whether the response asks.

#### Scenario: An offer without a question mark
- **WHEN** a response ends with `Если хочешь, сделаю это следующим шагом.`
- **THEN** it asks the person

#### Scenario: A link with a query
- **WHEN** the last sentence is `Logs: https://example.com/run?id=7`
- **THEN** that `?` does not make it ask

#### Scenario: A report
- **WHEN** a response ends with `Тестов 45, все проходят. Оба отчёта пересобраны.`
- **THEN** it does not ask

### Requirement: Words that ask
The words of A response that ask SHALL be these, in any case:
- permission: `твоего «да»`, `твоего «ок»`, `твоего одобрения`, `вашего ok`, `по твоей команде`, `по вашей команде`, `только после тво`, `your ok`, `your go-ahead`, `your approval`, `once you approve`;
- request words: `скажи`, `напиши`, `выбери`, `пришли`, `подтверди`, `ответь`, `дай знать` with their `-те` forms, `как пришлёшь`, `когда ответишь`, `когда решишь`, `let me know`, `tell me`, `choose`, `pick one`, `confirm`.

#### Scenario: Permission
- **WHEN** a response ends with ``/patronus merge` запущу только после твоего «да».`
- **THEN** it asks the person

### Requirement: Words that offer
The offers of A response that asks SHALL be: `если хочешь`, `если хотите`, `если нужно`, `если надо`, `если интересно`, `могу`, `можно сделать`, `можно добавить`, `можно заменить`, `можно попробовать`, `когда будете готовы`, `if you want`, `if you like`, `if you need`, `if you prefer`, `I can`, `want me to`, `happy to`, `when you're ready`. Each matches as whole words.

#### Scenario: An English offer
- **WHEN** a response ends with `I can split it into two commits if you want.`
- **THEN** it asks the person

### Requirement: What came after a turn
For each turn, tokprof SHALL take the person's next typed message in the same session after the turn's end, and put the time from the end to it in one of the ranges `under 1m`, `1–5m`, `5–15m`, `15–60m` and `1h+`. A turn with no later typed message in its session falls under `none`, and is that session's end. A wake or a turn the agent started itself in between does not change the range.

#### Scenario: Back in three minutes
- **WHEN** a turn ends at 10:04 and the person types in that session at 10:07
- **THEN** the turn's next is `1–5m`

#### Scenario: The last turn of a session
- **WHEN** a turn ends at 18:00, the person never types in that session again, and its last request was two days ago
- **THEN** the turn's next is `none`, and the session ended after that kind of turn

### Requirement: Open sessions
A session whose last request is less than one hour before the end of the selected time SHALL count as still open: its last turn falls under `none` but is not a session end.

#### Scenario: A session in use
- **WHEN** the selected time ends now, and a session's last turn ended 20 minutes ago
- **THEN** that turn's next is `none`, and the session counts in no session end

### Requirement: Stopped turns
For each `stopped` turn, tokprof SHALL keep its dollars, the time from its start to the stop, its edit calls, and the class of its last call before the stop, as cost-attribution classes a call. A turn that made no call has the class `no call`. It also keeps what came after the turn.

#### Scenario: Stopped during a shell call
- **WHEN** the person stops a turn 4 minutes after it started, while its last call ran `cargo test`, and types a new message 30 seconds later
- **THEN** the stopped turn has the class `building and testing`, 4m until the stop and the next `under 1m`

### Requirement: Run length
The turns the person started with a typed message SHALL be counted by their length, from start to end, in the ranges `under 1m`, `1–5m`, `5–15m`, `15m–1h` and `1h+`. For each range, tokprof keeps the turns, their dollars, edit calls, commits and stops. A turn whose typed message is the command `/goal` also counts among the goal runs, with its turns, dollars and edit calls.

#### Scenario: Two runs
- **WHEN** a person starts a 40-minute turn that makes 30 edit calls for $12, and a 2-minute turn that makes 1 edit call for $1.50
- **THEN** `15m–1h` holds 1 turn, $12 and 30 edit calls, and `1–5m` holds 1 turn, $1.50 and 1 edit call

#### Scenario: A goal
- **WHEN** a person types `/goal fix the flaky test` and the turn runs 25 minutes
- **THEN** it counts in `15m–1h` and among the goal runs
