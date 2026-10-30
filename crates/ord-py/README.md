# open-report-designer (Python)

Python bindings of Open Report Designer: one typed report document to PDF, HTML and Word.

```sh
pip install open-report-designer
```

The distribution is `open-report-designer`; the package you import is `ord_report`. Wheels are built for
Linux, macOS and Windows, for every Python from 3.10.

```python
import ord_report

pdf = ord_report.render_file("report.json", "pdf")
html = ord_report.render(document_dict, "html", assets={"plan": png_bytes})
findings = ord_report.check(document_dict)
```

A document with errors raises `ord_report.DocumentError`; its `findings` list says what and where.
