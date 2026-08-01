- Harness functionality

- Agentic Harness (a separate side tab on the left side like leaderboard, gorev panosu etc.)
- when you click inside you will see the leaderboard on the right, and submit your agent on the left
- there should be a switch between three leaderboards (ARC-AGI-3, Frontier-bench,RAM-bench), but you submit once to get your score for all leaderboards
- THIS IS TO SUBMIT ARC AGI 3 FOR THE LEADERBOARD
==================================================== 

The agent itself can be normal Python code. It does not need to be written as a Jupyter notebook.

Kaggle Code supports both:

.ipynb notebooks
.py script kernels

The official Kaggle CLI explicitly accepts a folder containing a .py, .ipynb, or .Rmd file alongside kernel-metadata.json.

The constraint is that the official evaluation must execute inside Kaggle Code. You cannot directly submit:

a GitHub repository;
a Docker image;
your own cloud endpoint;
a ZIP of arbitrary source code.

For your academy, students could work in a normal repository:

student-submission/
├── agent/
│   ├── my_agent.py
│   ├── planner.py
│   ├── memory.py
│   └── model.py
├── main.py
├── requirements.txt
└── kernel-metadata.json

main.py would simply act as the Kaggle entrypoint:

from agent.my_agent import MyAgent

agent = MyAgent()
agent.run()

Then upload the script-based Kaggle kernel:

kaggle kernels push -p student-submission/

ARC’s agent interface is already designed around ordinary Python files and classes; its official documentation instructs participants to create Python agent files and run them programmatically.
==============================================================
- 

- https://www.frontierbench.ai/run — start here, actual run instructions
- https://www.frontierbench.ai/announcement — what it is, scope, roadmap
- https://www.harborframework.com/docs/getting-started — Harbor, the engine it runs on
- https://github.com/laude-institute/terminal-bench-leaderboard — predecessor's submission format, likely the same shape
- https://www.tbench.ai/ — Terminal-Bench, still useful for task-authoring conventions

- https://docs.arcprize.org/arc-prize-2026 — the SDK. agent/my_agent.py, make play-local, make submit. This is the one that matters for you.
- https://arcprize.org/arc-agi/3 — what the benchmark tests
- https://arcprize.org/media/ARC_AGI_3_Technical_Report.pdf — design rationale, useful for teaching material
- https://arcprize.org/competitions/2026/arc-agi-3 — prize rules, ignore unless chasing money
- https://github.com/arcprize/ARC-AGI-Community-Leaderboard — submission format if you want students on a public board
- https://arcprize.org/leaderboard/community — existing entries, see what people built

- RAM BENCH
-----
RAM comparison
1 active session
Tool	PSS	Comparison
jcode (local embedding off)	27.8 MB	baseline
jcode	167.1 MB	6.0× more RAM
pi	144.4 MB	5.2× more RAM
Codex CLI	140.0 MB	5.0× more RAM
OpenCode	371.5 MB	13.4× more RAM
GitHub Copilot CLI	333.3 MB	12.0× more RAM
Cursor Agent	214.9 MB	7.7× more RAM
Claude Code	386.6 MB	13.9× more RAM
Antigravity CLI	243.7 MB	8.8× more RAM


10 active sessions
Tool	PSS	Comparison
jcode (local embedding off)	117.0 MB	baseline
jcode	260.8 MB	2.2× more RAM
pi	833.0 MB	7.1× more RAM
Codex CLI	334.8 MB	2.9× more RAM
OpenCode	3237.2 MB	27.7× more RAM
GitHub Copilot CLI	1756.5 MB	15.0× more RAM
Cursor Agent	1632.4 MB	14.0× more RAM
Claude Code	2300.6 MB	19.7× more RAM
Antigravity CLI	1021.2 MB	8.7× more RAM

we are looking to measure ram consumed during an active session too, the smaller it is the better it is

the kids will just link a url to their github code, and we will run it, but of course we need them to obey a certain structure, which you will write in a third tab within the agentic harness bar called `Instructions` where they will read and understand what they should do to create an agentic harness that obeys all submission rules we have, don't write any tests now you can seee if it compiles and builds but actual tests we will run later