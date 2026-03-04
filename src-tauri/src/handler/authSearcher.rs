use sqlite::{Connection, State};
use std::error::Error;
use serde::{Serialize};
use serde_json::json;

#[derive(Debug, Serialize)]
struct Account {
    username: String,
    uuid: String,
}

#[derive(Debug, Serialize)]
struct LittleSkinUser {
    name: String,
    tid_skin: i64,
    uuid: String,
}
/*读取操作示范
fn main() -> Result<(), Box<dyn Error>> {
    let connection = sqlite::open("D:/cpw/handler/src/LaunchAccount.db")?;

    // Get all accounts
    let account_users = get_all_accounts(&connection)?;
    // Get all LittleSkin users
    let littleskin_users = get_all_littleskin_users(&connection)?;

    // Create combined JSON output
    let output = json!({

        "littleskin_users": littleskin_users,
        "account":account_users
    });

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}
*/
fn get_all_accounts(connection: &Connection) -> Result<Vec<Account>, Box<dyn Error>> {
    let query = "SELECT username, uuid FROM accounts";
    let mut statement = connection.prepare(query)?;
    let mut accounts = Vec::new();

    while let State::Row = statement.next()? {
        accounts.push(Account {
            username: statement.read::<String, _>(0)?,
            uuid: statement.read::<String, _>(1)?,
        });
    }

    Ok(accounts)
}

fn get_all_littleskin_users(connection: &Connection) -> Result<Vec<LittleSkinUser>, Box<dyn Error>> {
    let query = "SELECT name, tid_skin, uuid FROM littleskinuser";
    let mut statement = connection.prepare(query)?;
    let mut users = Vec::new();

    while let State::Row = statement.next()? {
        users.push(LittleSkinUser {
            name: statement.read::<String, _>(0)?,
            tid_skin: statement.read::<i64, _>(1)?,
            uuid: statement.read::<String, _>(2)?,
        });
    }

    Ok(users)
}