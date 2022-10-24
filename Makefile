venv:
	python3 -m venv venv
	./venv/bin/pip install -r requirements.txt

test: venv
	./venv/bin/pip install -e .
	./venv/bin/pytest -v
