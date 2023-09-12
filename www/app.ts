import Chart from 'chart.js/auto';
import { getRelativePosition } from 'chart.js/helpers';


import { types } from "./json.js";

// GET and POST data relative to the current URL
const configDataUrl = window.location.href + "/config";
// const measurementDataURL = window.location.href + "/measurement.csv";

        
// for piecewiseConstant
function dupEnd(xs) {
  const [x, ...rest] = xs;
  return [x, ...rest.flatMap(y => [y, y])];
}
function dupStart(xs) {
  return dupEnd(xs.reverse()).reverse();
}
function zipArrays(arr1, arr2) {
  return arr1.map((val, index) => {
    return { x: val, y: arr2[index] };
  });
}

// the input/output arrays only describe the changes.
// Chart.js only draws sloped lines between points.
// There is no option to plot horizontal lines. So
// piecewiseConstant adds extra points and converts
// to the [{ x: _, y:_ }] format for type:"scatter".
function piecewiseConstant(xs, ys) {
  return zipArrays(dupEnd(xs), dupStart(ys));
}

function piecewiseConstantFromMap() {
        // get the second element of the map values
        const ys = Array.from(MAP.values()).map((curr) => curr[1]);
        const xs = Array.from(MAP.values()).map((curr) => curr[0]);
        return piecewiseConstant( xs, ys);
}

// Define the initial values of the function
let xValues = [0, 1, 2, 3, 5];
let yValues = [75, 60, 50, 40, 35];


// attempt to request the data from the server
// if the server is not running, use the default values above
// the server responds to XMLHttpRequests with a json object
// with the keys "x" and "y" which are arrays of numbers
function getData() {
        var request = new XMLHttpRequest();
        request.timeout = 1000;
        request.open("GET", configDataUrl, true);
        request.onload = () => {
                if (request.status == 200) {
                        // parse the json object and report an error if it is not valid
                        try { var data : types.Config = JSON.parse(request.responseText) } catch (e) {
                                // alert("The server did not respond with valid data. Using default values.");
                                return;
                        };
                        xValues = data.step_times;

                        for (const [i, v] of data.step_fracs.entries()) {
                                yValues[i] = v * 40 + 35;
                        }

                        // set the input values to the values received from the server
                        for (const n in ["w_cut", "n_wavelets", "period_ms"]) {
                         (document.getElementById(n) as HTMLInputElement).value = data[n].toString();
                        }

                        const xD = discretizeN('t', xValues);
                        xValues.map((curr, i) => { MAP.set(xD[i], [curr, yValues[i]]) });
                        sortCleanReplot();
                } else {
                        alert("The server did not respond in time. Using default values.");
                };

        };
        request.send();
};

// submit new configuration data to the server
// should include what was received by getData?
function submitData() {
        var request = new XMLHttpRequest();
        request.open("POST", configDataUrl, false);
        request.setRequestHeader("Content-Type", "application/json;charset=UTF-8");
        // TODO better initialization
        let st : types.Config["step_times"] = [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0];
        let sf : types.Config["step_fracs"] = [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0];
        
        for (const [i, v] of Array.from(MAP.values()).entries()) {
                st[i] = v[0];
                sf[i] = (v[1] - 35) / 40.0;
        }

        const f = (n,def) => parseFloat((document.getElementById(n) as HTMLInputElement).value) ?? def
        const response : types.Config = {
                step_times: st,
                step_fracs: sf,
                w_cut: f("w_cut",12),
                n_wavelets: f("n_wavelets", 40),
                measurement_period_ms : f("measurement_period_ms", 1000),
                last_modified: 0
        }
        request.send(JSON.stringify(response));
};

// global map from rounded time values to [time, Temperature] pairs
var MAP = new Map();

// if n is "T" then we use T_divs and T_max, if n is "t" then we use t_divs and t_max in the call to discretize
function discretizeN(n, xs) {
        const divs = document.getElementById(n + "_divs") as HTMLInputElement;
        const max = document.getElementById(n + "_max")   as HTMLInputElement;
        const min = document.getElementById(n + "_min")   as HTMLInputElement;
        let minVal = 0;
        if (min != null) minVal = parseFloat(min.value);
        return discretize(parseInt(divs.value),
                parseInt(max.value),
                minVal,
                xs);
}

// make an integer array with values on [0,divs] from a float array xs
// which is assumed to be on [min,max]
function discretize(divs : number, max : number, min : number, xs : number[]) : number[]{
        var out : number[] = Array();
        // ensure divs is an integer
        divs = Math.round(divs);
        for (const x of xs) {
                if (x >= max) {
                        out.push(divs);
                } else if (x <= min) {
                        out.push(0);
                } else {
                        out.push(Math.round((0.0 + x-min) * divs / max));
                };
        };
        return out;
};

{
  let xD = discretizeN("t", xValues);
  xValues.map((curr, i) => {
          MAP.set(xD[i], [curr, yValues[i]])
  });
}

function insertOrDelete(x, y) {
  var xDiscretized = discretizeN("t", [x]);
  var xD = xDiscretized[0];
  if (MAP.has(xD)) {
    MAP.delete(xD);
  } else {
    MAP.set(xD, [x,y]);
  }
}

// TODO:
// - axis labels
// - drag and touch events
// - disable or improve animation on update: when deleting or adding a point
//   the old points all move over one index left or right
const TtChartElem = document.getElementById("TtChart") as HTMLCanvasElement;
getData();
const TtChart = new Chart(TtChartElem, {
  type: "scatter",
  options: {
          scales: { x: { min: 0, max : Math.round((document.getElementById("t_max") as HTMLInputElement).value ?? "100"),
                                title : { display: true, text: "Time (hours)" } },
                    y: { min: 35, max : 75,
                        title : { display: true, text: "Temperature (°C)" } }},
        onClick: (e) => {
            const canvasPosition = getRelativePosition(e, TtChart);

            // Substitute the appropriate scale IDs
            const dataX = TtChart.scales.x.getValueForPixel(canvasPosition.x);
            const dataY = TtChart.scales.y.getValueForPixel(canvasPosition.y);

            insertOrDelete(dataX, dataY);
            sortCleanReplot();
            console.log(dataX,dataY);

        }
  },
  data: {
    datasets: [{
      label: "temperature setpoint",
      data: piecewiseConstantFromMap(),
      borderColor: "red",
      borderWidth: 2,
      showLine: true,
      fill: false
    }]
  },
});

// removes redundant points which do not affect the piecewise constant function
// represented by the map
function clean(map) {
        var vOld = {};
        var firstIter = true;
        var deletes = [];
        const kMax = Math.max(...Array.from(map.keys()));
        const kMin = Math.min(...Array.from(map.keys()));
        for (const entry of map) {
                const value = entry[1][1];
                const key = entry[0];
                if (firstIter) {
                        vOld = value;
                        firstIter = false;
                        continue;
                };
                let oT = discretizeN("T", [vOld]);
                let nT = discretizeN("T", [value]);

                if (oT[0] == nT[0]) {
                        deletes.push(key);
                };
                if (key == kMax) continue;
                vOld = value;
                kOld = key;
        };
        for (const key of deletes) {
                if (key != kMax && key != kMin) { map.delete(key); }

        };
};


// reorders the map, removes redundant points, and updates the chart
function sortCleanReplot() {
  // array of [discretized time, [time, temperature]] sorted by time
  let newAssocs = [...MAP.values()]
                .sort((a, b) => a[0] - b[0])
                .map(v => [discretizeN("t", [v[0]])[0], v])
  // if the first point has the same discretized time as the second,
  // MAP will contain only the second point
  MAP = new Map(newAssocs);
  let head = newAssocs[0]; // first point
  MAP.delete(head[0]);
  newAssocs = [...MAP.entries()];
  newAssocs.unshift(head);
  MAP = new Map(newAssocs);
  // now MAP contains the first point again
  // the last point is correctly retained
  // and insertion/iteration order is correct

  clean(MAP);
  TtChart.data.datasets[0].data = piecewiseConstantFromMap();
  TtChart.update();
}


// report the size of the division in °C to the user on the label for the slider
setTStep = () =>
        document.getElementById("T_divs_label").innerHTML = "temperature steps " + Math.round((document.getElementById("T_max").value - document.getElementById("T_min").value) / document.getElementById("T_divs").value * 10)/10 + " °C" ;
setTStep();

settStep = () =>
        document.getElementById("t_divs_label").innerHTML = "time steps " + Math.round((document.getElementById("t_max").value) / document.getElementById("t_divs").value * 10)/10 + " hours" ;

settStep();

// call sortCleanReplot when applicable slider inputs change
document.getElementById("T_divs").addEventListener("input", () => {
        sortCleanReplot();
        setTStep();
});
document.getElementById("T_max").addEventListener("input", () => {
        const T_max = Math.round(document.getElementById("T_max").value);
        TtChart.options.scales.y.max = T_max;
        // reduce temperatures in MAP if necessary
        MAP.forEach(v => v[1] > T_max ? v[1] = T_max : v[1]);
        sortCleanReplot();
});
document.getElementById("T_min").addEventListener("input", () => {
        const T_min = Math.round(document.getElementById("T_min").value);
        TtChart.options.scales.y.min = T_min;
        // increase temperatures in MAP if necessary
        MAP.forEach(v => v[1] < T_min ? v[1] = T_min : v[1]);
        sortCleanReplot();
});
document.getElementById("t_divs").addEventListener("input", () => {
        sortCleanReplot();
        settStep();
});
document.getElementById("t_max").addEventListener("input", () => {
        sortCleanReplot();
        TtChart.options.scales.x.max = Math.round(document.getElementById("t_max").value);
});

// ensure T_min is 5 less than T_max
document.getElementById("T_max").addEventListener("input", () => {
        var T_max = document.getElementById("T_max").value;
        var T_min = document.getElementById("T_min").value;
        if (T_max - T_min < 5) {
                document.getElementById("T_min").value = T_max - 5;
        };
});
// ensure T_max is 5 more than T_min
document.getElementById("T_min").addEventListener("input", () => {
        var T_max = document.getElementById("T_max").value;
        var T_min = document.getElementById("T_min").value;
        if (T_max - T_min < 5) {
                document.getElementById("T_max").value = T_min + 5;
        };
});



function submitCalibration() {
        // values from the calibration form
        let y1 = document.getElementById("calibration_y1").value;
        let y2 = document.getElementById("calibration_y2").value;
        const s1 = document.getElementById("calibration_save1").value;
        const s2 = document.getElementById("calibration_save2").value;
        // not sure if this is needed
        if (y1 == "") y1 = null;
        if (y2 == "") y2 = null;
        const data = {
                "y": [y1, y2],
                "save": [s1, s2]
        }
        // submit POST request
        var request = new XMLHttpRequest();
        request.open("POST", "/calib", true);
        request.setRequestHeader("Content-Type", "application/json");
        request.send(JSON.stringify(data));
}

// retrieve calibration data from server with a GET to /calib,
// and store it in the calibration form
// not quite right because calibration data is a pair of x and y points
// the server sends an array of objects with x0 x1 y0 and y1 properties
function getCalibration() {
        var request = new XMLHttpRequest();
        request.open("GET", "/calib", true);
        request.onload = function() {
                const data = JSON.parse(this.response);
                function to_str(obj) { `x,y=${obj.x0},${obj.y0}; x,y=${obj.x1},${obj.y1}` };
                document.getElementById("calibration_1").value = to_str(data[0]);
                document.getElementById("calibration_2").value = to_str(data[1]);
        };
        request.send();
}
