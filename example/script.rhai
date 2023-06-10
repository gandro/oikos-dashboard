fn format_date(date) {
    let month_day = date.month_day;
    let month = switch date.month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
    };

    `${month} ${month_day}`
}

fn format_weekdate(date) {
    switch date.week_day {
        0 => "Sunday",
        1 => "Monday",
        2 => "Tuesday",
        3 => "Wednesday",
        4 => "Thursday",
        5 => "Friday",
        6 => "Saturday",
    }
}

fn format_number(n) {
    if n < 10 {
        `0${n}`
    } else {
        `${n}`
    }
}

let now = datetime::datetime();

document.id("weekday").text(format_weekdate(now));
document.id("date").text(format_date(now));

document.id("hour").text(format_number(now.hour));
document.id("minute").text(format_number(now.minute));

const repo = "gandro/oikos-dashboard";
document.id("repo_text").text(repo);

let issue = fetch(`https://api.github.com/search/issues?q=is:issue+repo:${repo}+is:open&per_page=1`);
document.id("issue_text").text(`${issue.total_count}`);

let pr = fetch(`https://api.github.com/search/issues?q=is:pr+repo:${repo}+is:open&per_page=1`);
document.id("pr_text").text(`${pr.total_count}`);

let gh = fetch(`https://api.github.com/repos/${repo}`);
document.id("star_text").text(`${gh.stargazers_count}`);
document.id("fork_text").text(`${gh.forks_count}`);

document.class("centered").align(alignment::CENTER);
