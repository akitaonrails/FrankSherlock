## Frank Sherlock

I want to figure out if modern LLMs and open-source vision models can classify media, not just text. There are tools that can open a .jpg of an anime character and say "young woman on a beach." But can they narrow it down to "the character Ranma, from Rumiko Takahashi's Ranma 1/2, wearing a bikini on the beach from the OVA The Battle for Miss Beachside"? I genuinely don't know.

Then there's audio. How far have we come with Shazam-like recognition? I know there are fingerprinting algorithms that match songs against a database (are there open databases?) from just a few seconds. Worth looking into.

For video, we have short clips and full movies. Is there a way to identify a movie without reading the entire multi-gigabyte stream? Maybe from the audio track with a Shazam-like approach, or from a few sampled frames?

I've put together a `test_files` directory with a bunch of different media files. I want to survey what's out there for cataloging this kind of stuff. If there are multiple viable options, we'll A/B test them.

Hardware: AMD 7850X3D CPU, RTX 5090 GPU, Arch Linux. No remote APIs — no OpenRouter, no OpenAI. Everything has to be open-source and run locally. If this works, the next project is cataloging terabytes of files on my home NAS.

First things first though: the research has to pan out before we build anything real. Small scripts, quick prototypes, different models. ImageMagick and FFmpeg for file manipulation. See what sticks.
