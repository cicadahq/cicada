from typing import Callable, Dict, List, Literal, Optional, Union
import uuid
import os

# A file path represented as a string.
FilePath = str


class CacheDirectoryOptions:
    """
    Options for a cached directory, including the path and sharing options.
    """

    def __init__(
        self,
        path: FilePath,
        sharing: Optional[Literal["shared", "private", "locked"]] = None
    ) -> None:
        self.path = path
        self.sharing = sharing

    def __repr__(self) -> str:
        return f"CacheDirectoryOptions(path={self.path}, sharing={self.sharing})"


# A directory to cache, which can be a single file path or an array of file paths.
CacheDirectories = List[Union[FilePath, CacheDirectoryOptions]]

StepFn = Callable


class Step:
    """
    A step in the pipeline, which can be an object with a name and a run property,
    a step function, or a string command.
    """

    def __init__(
        self,
        run: Union[str, StepFn],
        name: Optional[str] = None,
        cacheDirectories: Optional[CacheDirectories] = None,
        ignoreCache: Optional[bool] = None,
        env: Optional[Dict[str, str]] = None,
        secrets: Optional[List[str]] = None,
        workingDirectory: Optional[FilePath] = None,
    ) -> None:
        self.run = run
        self.name = name
        self.cacheDirectories = cacheDirectories
        self.ignoreCache = ignoreCache
        self.env = env
        self.secrets = secrets
        self.workingDirectory = workingDirectory

    def __repr__(self) -> str:
        return f"Step(run={self.run}, name={self.name}, cacheDirectories={self.cacheDirectories}, ignoreCache={self.ignoreCache}, env={self.env}, secrets={self.secrets}, workingDirectory={self.workingDirectory})"


class Job:
    """
    Represents a job in the pipeline with its options.
    """

    def __init__(
        self,
        image: str,
        steps: List[Union[Step, StepFn, str]],
        name: Optional[str] = None,
        env: Optional[Dict[str, str]] = None,
        cacheDirectories: Optional[CacheDirectories] = None,
        workingDirectory: Optional[FilePath] = None,
        onFail: Optional[Literal["ignore", "stop"]] = None
    ) -> None:
        """
        Creates a new Job instance.

        :param options: The options for the job.
        """
        self.image = image
        self.steps = steps
        self.name = name
        self.env = env
        self.cacheDirectories = cacheDirectories
        self.workingDirectory = workingDirectory
        self.onFail = onFail
        self._uuid = uuid.uuid4()
    
    def __repr__(self) -> str:
        return f"Job(image={self.image}, steps={self.steps}, name={self.name}, env={self.env}, cacheDirectories={self.cacheDirectories}, workingDirectory={self.workingDirectory}, onFail={self.onFail}, _uuid={self._uuid})"


class Pipeline:
    """
    Represents a pipeline with an array of jobs.
    """

    def __init__(self, jobs: List[Job]) -> None:
        """
        Creates a new Pipeline instance.

        :param jobs: The jobs to include in the pipeline.
        """
        self.jobs = jobs

    def __repr__(self) -> str:
        return f"Pipeline(jobs={self.jobs})"


def inJob() -> bool:
    val = os.environ.get("CICADA_JOB")
    return val is not None and val != ""
