//This project implements a flashcards app with an optional console interface written using the cursive library. This file defines two structs: Facet (representing an underlying latent concept) and Gem (a flashcard which can feature several facets). When a Gem is reviewed, the user marks which of the facets they got correct. For instance, the Gem for the sentence "The quick brown fox jumps over the lazy dog" might have the facets "quick", "brown", "fox", "jumps", "over", and "lazy". The user might mark the "quick" facet as correct, and the "lazy" facet as incorrect. The Gem will then update the review dates of its facets based on this.

//Imports from Rust's standard library and says we're allowed to use unused imports
#[allow(unused_imports)]
use std::{
    collections::HashMap,
    fmt::{self, Display},
    io::{self, Write},
    ops::{Add, Sub},
    time::{Duration, Instant},
    task::{Context, Poll, Waker},
    sync::{Arc, Mutex},
    fs::{File},
};
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};


#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Facet {
    pub name: String,
    pub review_date: Option<SystemTime>,
    pub last_seen_date: Option<SystemTime>,
    pub lifetime_in_hours: Option<f64>,
    pub stage: Option<String>,
}

//The binary method for updating a Facet based on whether a user's response was right or wrong is simple.

impl Facet {
    pub fn update_facet_binary(&mut self, correct: bool) {
        //First, we want to throw an error if any of the fields are None:
        if self.review_date.is_none() || self.last_seen_date.is_none() || self.lifetime_in_hours.is_none() {
            panic!("Facet {} has a None value in one of its fields. This is a bug.", self.name);
        }
        //Now we calculate the number of hours since last_seen_date and now:
        let hours_since_last_seen = match self.last_seen_date {
            Some(last_seen_date) => {
                let now = SystemTime::now();
                let duration = now.duration_since(last_seen_date).unwrap();
                duration.as_secs() as f64 / 3600.0
            },
            None => 0.0,
        };
        //We then check if hours_since_last_seen is greater than lifetime_in_hours. If it is, we set the lifetime_in_hours to 3 * hours_since_last_seen.
        if correct {
            let new_lifetime_in_hours = match self.lifetime_in_hours {
                Some(lifetime_in_hours) => {
                    if hours_since_last_seen > lifetime_in_hours {
                        3.0 * hours_since_last_seen
                    } else {
                        if lifetime_in_hours > (0.05 * 27.0) - 1.0 {
                            lifetime_in_hours + hours_since_last_seen
                        } else {
                            hours_since_last_seen * 3.0
                        }
                    }
                },
                None => 0.0,
            };
            self.lifetime_in_hours = Some(new_lifetime_in_hours);
        } else {
            self.lifetime_in_hours = Some(&self.lifetime_in_hours.unwrap() / 3.0);
        }
        //Then, we move forward the review date:
        let new_review_date = match self.review_date {
            Some(review_date) => {
                let now = SystemTime::now();
                let duration = now.duration_since(review_date).unwrap();
                let hours_since_review = duration.as_secs() as f64 / 3600.0;
                let new_hours_since_review = hours_since_review + self.lifetime_in_hours.unwrap();
                let new_review_date = review_date + Duration::from_secs((new_hours_since_review * 3600.0) as u64);
                Some(new_review_date)
            },
            None => None,
        };
        self.review_date = new_review_date;
        //And finally, we set last_seen to now:
        self.last_seen_date = Some(SystemTime::now());
    }
    //The 'fuzzy' method, which receives a number between 0 and 1, simply clones the Facet twice, calls update_facet_binary on them with correct = true and correct = false respectively, then creates a weighted average of the two Facets' fields. After that, it sets its own attributes to the average.
    pub fn update_facet_fuzzy(&mut self, correct: f64) {
        let mut facet_1 = self.clone();
        let mut facet_2 = self.clone();
        facet_1.update_facet_binary(true);
        facet_2.update_facet_binary(false);
        *self = self.average_facet_fields(facet_1, facet_2, correct)
    }
    pub fn average_facet_fields(&self, facet_1: Facet, facet_2: Facet, correct: f64) -> Facet {
        let mut new_facet = Facet {
            name: self.name.clone(),
            review_date: None,
            last_seen_date: Some(SystemTime::now()),
            lifetime_in_hours: None,
            stage: None,
        };
        let ratios = [correct, 1.0 - correct];
        let review_date = facet_1.review_date.unwrap().duration_since(UNIX_EPOCH).unwrap().as_secs() as f64 * ratios[0] + facet_2.review_date.unwrap().duration_since(UNIX_EPOCH).unwrap().as_secs() as f64 * ratios[1];
        new_facet.review_date = Some(UNIX_EPOCH + Duration::from_secs(review_date as u64));
        let lifetime_in_hours = facet_1.lifetime_in_hours.unwrap() * ratios[0] + facet_2.lifetime_in_hours.unwrap() * ratios[1];
        new_facet.lifetime_in_hours = Some(lifetime_in_hours);
        new_facet
    }
}

//Method for reading and writing a list of facets to and from a json file:
impl Facet {
    pub fn read_from_file(filename: &str) -> Vec<Facet> {
        //Compact method
        let mut file = File::open(filename).expect("File not found");
        let mut contents = String::new();
        file.read_to_string(&mut contents).expect("Could not read file");
        let facets: Vec<Facet> = serde_json::from_str(&contents).expect("Could not parse json");
        facets
    }
    pub fn write_to_file(&self, filename: &str) {
        let mut file = File::create(filename).expect("File not found");
        let contents = serde_json::to_string(&self).expect("Could not serialize json");
        file.write_all(contents.as_bytes()).expect("Could not write to file");
    }
}
//Gem will have fields called: sides, all_facets, and unknown_facets. 'Sides' is simply a dictionary where keys are integers and values are strings. 'All facets' is a list of all the facets in the Gem. 'Unknown facets' is a list of facets that have not been reviewed yet. The latter two are implemented as HashMaps to allow for facets to be efficiently stripped from a very large list of Gems.

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Gem {
    pub sides: HashMap<i32, String>,
    pub unknown_facets: HashMap<String, Facet>,
}
//Method for reading and writing a list of gems to and from a json file:
impl Gem {
    pub fn read_from_file(filename: &str) -> Vec<Gem> {
        //Compact method
        let mut file = File::open(filename).expect("File not found");
        let mut contents = String::new();
        file.read_to_string(&mut contents).expect("Could not read file");
        let gems: Vec<Gem> = serde_json::from_str(&contents).expect("Could not parse json");
        gems
    }
    pub fn write_to_file(&self, filename: &str) {
        let mut file = File::create(filename).expect("File not found");
        let contents = serde_json::to_string(&self).expect("Could not serialize json");
        file.write_all(contents.as_bytes()).expect("Could not write to file");
    }
}
//This means that in order for serde_json to work, the json file will need to look exactly like this:

//Implementation for constructing a Gem from a HashMap of sides, and a Vec of strings which are the names of the facets.
impl Gem {
    pub fn new(sides: HashMap<i32, String>, facets: Vec<String>) -> Self {
        let mut unknown_facets = HashMap::new();
        for facet in facets {
            let facet_name = facet.clone();
            unknown_facets.insert(facet_name, Facet {
                name: String::from(&facet),
                review_date: None,
                lifetime_in_hours: None,
                last_seen_date: None,
                stage: None,
            });
        }
        Gem {
            sides,
            unknown_facets,
        }
    }
}

//Implements a .clone trait for Gems and Facets
impl Clone for Gem {
    fn clone(&self) -> Self {
        Gem {
            sides: self.sides.clone(),
            unknown_facets: self.unknown_facets.clone(),
        }
    }
}


impl Clone for Facet {
    fn clone(&self) -> Self {
        Facet {
            name: self.name.clone(),
            review_date: self.review_date.clone(),
            lifetime_in_hours: self.lifetime_in_hours.clone(),
            last_seen_date: self.last_seen_date.clone(),
            stage: self.stage.clone(),
        }
    }
}

//This function iterates through a Vec of Gems and returns a Vec of all Gems with a specific number of unknown factes (e.g 1 or 2)
pub async fn get_gems_with_n_unknown_facets(
    gems: &[Gem],
    n: usize,
) -> Vec<&Gem> {
    let mut gems_with_n_unknown_facets = Vec::new();
    for gem in gems {
        if gem.unknown_facets.len() == n {
            gems_with_n_unknown_facets.push(gem);
        }
    }
    gems_with_n_unknown_facets
}


//This function takes a reference to a Vec of Gems, calls the above function to get a Vec of Gems containing just 2 unknown facets, and returns a HashMap. The HashMap has a string representing a Facet, and an integer representing the number of times the Facet occurred in the 2-unknown Vec of Gems.
pub async fn get_facet_counts(gems: &[Gem], n: usize) -> HashMap<String, i32> {
    let gems_with_two_unknown_facets = get_gems_with_n_unknown_facets(gems, n).await;
    let mut facet_counts = HashMap::new();
    for gem in gems_with_two_unknown_facets {
        for facet in gem.unknown_facets.values() {
            let facet_name = facet.name.clone();
            let facet_count = facet_counts.entry(facet_name).or_insert(0);
            *facet_count += 1;
        }
    }
    facet_counts
}

//This function takes a reference to a Vec of Gem. It gets a Vec containing only Gems with n unknown Facet called gems_with_one_unknown_facet. Then it calls get_facet_counts. It implements an internal function that sorts gems_with_one_unknown_facet according to the Hashmap, and returns the sorted Vec.
pub async fn get_gems_with_n_unknown_facets_sorted(
    gems: &[Gem],
    n: usize,
) -> Vec<&Gem> {
    let gems_with_one_unknown_facet = get_gems_with_n_unknown_facets(gems, n).await;
    let facet_counts = get_facet_counts(gems, n+1).await;
    let mut sorted_gems_with_one_unknown_facet = Vec::new();
    for gem in gems_with_one_unknown_facet {
        sorted_gems_with_one_unknown_facet.push(gem);
    }
    sorted_gems_with_one_unknown_facet.sort_by(|a, b| {
        let facet_count_a = facet_counts.get(&a.unknown_facets.values().next().unwrap().name);
        let facet_count_b = facet_counts.get(&b.unknown_facets.values().next().unwrap().name);
        facet_count_b.cmp(&facet_count_a)
    });
    sorted_gems_with_one_unknown_facet
}

//This function wraps the above. It starts with n=1, and if the returned list is empty, then it increments upwards until the list is not empty.
pub async fn get_gems_with_n_unknown_facets_sorted_wrapper(
    gems: &[Gem],
) -> Vec<&Gem> {
    let mut n = 1;
    let mut sorted_gems_with_one_unknown_facet = get_gems_with_n_unknown_facets_sorted(gems, n).await;
    
    while sorted_gems_with_one_unknown_facet.is_empty() {
        n += 1;
        sorted_gems_with_one_unknown_facet = get_gems_with_n_unknown_facets_sorted(gems, n).await;
    }
    
    sorted_gems_with_one_unknown_facet
}

//This function takes a HashMap of known Facets, and a Vec of Gems, and strips the known Facets from each Gem's unknown_facets HashMap in a moderately effficient way.
pub async fn strip_known_facets(
    known_facets: HashMap<String, Facet>,
    gems: Vec<Gem>,
) -> Vec<Gem> {
    let mut stripped_gems = Vec::new();
    for gem in gems {
        let mut stripped_gem = gem.clone();
        for facet in gem.unknown_facets.values() {
            let facet_name = facet.name.clone();
            if known_facets.contains_key(&facet_name) {
                stripped_gem.unknown_facets.remove(&facet_name);
            }
        }
        stripped_gems.push(stripped_gem);
    }
    stripped_gems
}
//However, we can take advantage of caching here and implement it this way instead:

    

pub async fn order_gems_by_difficulty(all_gems: Vec<Gem>) {
    //This function starts with an empty hashmap called known_facets. It calls get_gems_with_1_unknown_facets_sorted using all_gems. From that, it gets the top Gem, and updates the known_facets hashmap with the Facet of that Gem. It then calls strip_known_facets, and returns the stripped Gems. Those gems are used to call get_gems_with_1_unknown_facets_sorted again, and so on.
    let mut known_facets: HashMap<String, Facet> = HashMap::new();
    let mut gems_with_known_facets_stripped = all_gems.clone();
    for _ in 0..200 {
        let top_gem = &*get_gems_with_n_unknown_facets_sorted_wrapper(&gems_with_known_facets_stripped).await.first().unwrap().clone();
        //Inserts the name of every facet in the top gem into the known_facets hashmap
        for facet in top_gem.unknown_facets.values() {
            let facet_name = facet.name.clone();
            known_facets.insert(facet_name, facet.clone());
        }
        //Prints the first side of the gem for debugging purposes:
        //println!("{:?}", top_gem.sides.values().next().unwrap());
        let start = Instant::now();
        gems_with_known_facets_stripped = strip_known_facets(known_facets.clone(), gems_with_known_facets_stripped).await;
        let finish = Instant::now();
        println!("{:?}", finish.duration_since(start));

    }
}

//Now, our main function:
#[tokio::main]
async fn main() {
    //Deserialises a Vec of Gems from gems.json, times the time it takes to do order_gems_by_difficulty, and prints the time
    let all_gems: Vec<Gem> = serde_json::from_str(include_str!("gems.json")).unwrap();
    //Print the first Gem for testing purposes:
    println!("{:?}", all_gems.first().unwrap());
    //All of the gems' facets will have None as review_date, and None as lifetime_in_hours, which will cause .unwrap() to panic. We'll set all of the facets in all of the gems to an empty string:
    let new_gems: Vec<Gem> = all_gems.into_iter().map(|gem| {
        let mut new_gem = gem;
        for facet in new_gem.unknown_facets.values_mut() {
            facet.review_date = None;
            facet.lifetime_in_hours = None;
            facet.last_seen_date = None;
            facet.stage = Some("new".to_string());
        }
        new_gem
    }).collect();
    //Print the first Gem for testing purposes:
    println!("{:?}", new_gems.first().unwrap());
    let start = Instant::now();
    order_gems_by_difficulty(new_gems).await;
    let end = Instant::now();
    let duration = end.duration_since(start);
    println!("{}", duration.as_secs_f64());
    //The problem is that we can't call order_gems like this since it's an async function. To do that, we can rewrite the above as:
    //let all_gems 

}
