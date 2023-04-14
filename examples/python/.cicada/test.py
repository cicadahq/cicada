from a import Job, Step, Pipeline

job = Job(
    name="Simple job",
    image="ubuntu:latest",
    steps=[
        Step(
            name="Print a message",
            run="echo Hello, world!",
        ),
        "ls -al /usr/local/bin",
        "pwd",
        Step(
            name="Run a js function",
            run=lambda: print("Hello from js"),
        ),
    ],
)

pipeline = Pipeline([job])
