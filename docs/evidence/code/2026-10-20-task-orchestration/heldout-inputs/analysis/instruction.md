# Regional shipment report

Read every CSV under regions/. Count only rows whose status is delivered. Sum integer units for each region and across all regions. Save report.json with exactly schema_version="regional-units-v1", regions (objects containing region and delivered_units, sorted by region), and total_delivered_units. Do not modify source files. Validate the saved report and report its relative path.
