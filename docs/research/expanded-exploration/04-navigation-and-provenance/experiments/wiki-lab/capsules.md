# Generated capsules: raw experimental outputs

These outputs are retained for inspection, including omissions and distortions. They are not accepted wiki knowledge. Source IDs resolve in portable-fixtures.json; full audit is in analysis.json.

## motion-full: Controlled Motion Detection Demonstration

A controlled research demonstration used the public OpenCV surveillance sample to detect frame changes by marking changed pixels without interpreting people or their intent. [motion-c1]; status: recorded.
The detection method was updated to use grayscale conversion, a small Gaussian blur, and a threshold of 35 to reduce noise compared to a direct RGB comparison with threshold 10. [motion-c2, motion-c3]; status: recorded.
The difference masks produced indicate pixel changes but are not reliable for object tracking or identifying suspicious behavior. [motion-c4, motion-c3]; status: recorded.
An example measurement at 2 seconds showed 11,566 changed pixels out of 442,368 total pixels, demonstrating the pixel change detection output. [motion-t2]; status: recorded.

## motion-lossy: Motion Detection Method Using Grayscale and Thresholding

The motion detection process uses grayscale images with a small Gaussian blur and a threshold value of 35 to create difference masks. [motion-c3, motion-code]; status: recorded.
Frame time anchors are maintained during the processing to preserve temporal alignment. [motion-c3]; status: recorded.
In one measurement, over 2 seconds, 11,566 pixels changed out of a total of 442,368 pixels. [motion-t2]; status: recorded.

## book-full: Natural Selection and Selection Operators in Evolutionary Algorithms

Natural selection preserves favorable variations and rejects injurious ones, while neutral variations remain fluctuating elements in populations, especially under changing environmental conditions such as climate shifts. [book-ocr]; status: recorded.
Selection operators in the DEAP evolutionary computation framework select individuals from a population based on criteria like fitness, without duplicating individuals during the selection process. [deap-article]; status: recorded.
Common selection methods implemented in DEAP include random selection, best and worst selection, tournament selection, roulette wheel selection, and double tournament selection, each with specific mechanisms and use cases. [deap-code]; status: recorded.
Double tournament selection in DEAP uses both fitness and individual size to reduce bloat in genetic programming, with parameters controlling tournament sizes and order, as described in Luke and Panait (2002). [deap-code]; status: recorded.

## book-lossy: Natural Selection and Selection Operators in Evolutionary Processes

Natural selection involves the preservation of favorable variations and the rejection of injurious variations. [book-ocr]; status: recorded.
Selection operators function by selecting individuals as references rather than creating independent copies. [deap-article]; status: recorded.
Retaining only the fastest benchmark configuration may result in losing alternative candidates that could be useful under changed workloads. [controlled-note]; status: recorded.
It remains tentative whether keeping a diverse set of candidates improves adaptation to varying conditions. [controlled-note]; status: tentative.

## methods-full: Preserving Distinct Source Identities in Lookup Keys

Preserving exact identity and scope fields in lookup keys prevents unintended collisions, as shown by failed attempts to normalize numeric IDs or URL components that collapsed distinct values. [thread-csv_ids, thread-url_keys, thread-cache_scope]; status: recorded.
Regression tests that reproduce collisions and then apply bounded corrections have been used to verify that distinct source identities are maintained in key derivation. [thread-csv_ids, thread-url_keys, thread-cache_scope]; status: recorded.
Some exceptions exist, such as numeric source fields where normalization of padding may be legitimate, and tenant identity is not relevant for public immutable resources. [method-candidate]; status: tentative.
The question of sorting query parameters for signed URLs remains unresolved due to lack of provider signing specifications and verification, so original bytes should be preserved until semantics are confirmed. [thread-unresolved, method-candidate]; status: tentative.

## methods-lossy: Data Processing Methods for ID and Key Handling

Padded IDs were initially collapsed due to integer conversion, but subsequent correction passes preserved their string format. [thread-csv_ids]; status: recorded.
URL keys without queries were collapsed into distinct views, while keys preserving queries passed correction processes. [thread-url_keys]; status: recorded.
Keys without tenant or policy context were collapsed across scopes, but partitioned keys successfully passed correction. [thread-cache_scope]; status: recorded.

