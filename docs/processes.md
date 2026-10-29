# Processes

The Processes window shows every process SnoutCode runs for you, says what each one is for, and lets you stop or restart them.

## Open the Processes window

- Choose **View ▸ Processes**.
- Or click the memory figure in the status bar. Its tooltip gives the number of processes and the memory they take.
- Or run **Show Processes** from the command palette (⇧⌘P).

The window opens in the bottom dock and looks at the processes again every few seconds.

## What it lists

The processes show as a tree under SnoutCode itself:

- **SnoutCode**: the window, the editors and Git.
- **Java engine** for each project: it reads the Gradle build, runs and tests the code.
- **Java language server** under its engine: completion, errors, navigation.
- **Gradle daemons** of the engine, with what they start: test JVMs, Gradle workers, and programs Gradle started (as `bootRun` does).
- **Your programs**, run or debugged from the editor.
- **Terminals**, the **Claude Code** console, and **Git** commands that are running.
- Processes **left over from an earlier session** (see below).

Each line says what the process is for, and its columns give its **PID**, **Memory**, **CPU**, **Uptime** and **State**. Hover a line to see its project. A Gradle daemon's state is **Building**, or how long it has been idle and when it stops.

The foot of the window gives the totals: processes, memory and CPU.

## Stop or restart a process

Each line offers the buttons that make sense for it:

| Process | Buttons |
|---|---|
| Java engine | **Restart**, **Stop** |
| Java language server | **Restart** (restarts its engine) |
| A program you ran or debugged | **Restart**, **Stop** |
| Gradle daemon, test JVM, Gradle worker, leftover | **Stop** |

To restart the Java engine of the open project without the window, run **Restart the Java Engine** from the command palette.

## Stop idle processes

Click **Stop Idle Processes** at the foot of the window, or in its header, or run it from the command palette. It stops:

- the Gradle daemons that are not building,
- what an earlier session left running,
- the Java engines of projects that are not open.

The foot then says how many it stopped, or **Nothing idle to stop.**

The engine's Gradle daemons also stop by themselves after a while without a build. To change how long, see [Gradle](gradle.md#gradle-daemons).

## Leftovers from an earlier session

If SnoutCode crashed or was killed, its Java engines, their language servers and their Gradle daemons can outlive it. When SnoutCode starts, it stops what an earlier session left running: engines and language servers whose SnoutCode is gone, and Gradle daemons that no running engine owns.

Anything it finds later shows in the Processes window as **left over from an earlier session**, marked in a warning colour. Stop it with its **Stop** button, or with **Stop Idle Processes**.

SnoutCode never touches processes it did not start: a Gradle daemon you started from a terminal, or the engines of another SnoutCode window, are left alone.
