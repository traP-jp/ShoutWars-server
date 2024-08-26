#pragma once

#include "sync_record.hpp"

#include <nlohmann/json.hpp>
#include <boost/uuid.hpp>
#include <chrono>
#include <condition_variable>
#include <shared_mutex>
#include <map>
#include <string>
#include <vector>
#include <functional>
#include <memory>

class room_t {
public:
  // Note that although user_t is mutable, it is not thread-safe.
  class user_t {
  public:
    static constexpr size_t name_max_length = 32;

    const boost::uuids::uuid id;

    [[nodiscard]] explicit user_t(const std::string &name);

    friend void to_json(nlohmann::json &j, const user_t &user);

    [[nodiscard]] std::string get_name() const;

    void set_name(const std::string &new_name);

    [[nodiscard]] boost::uuids::uuid get_last_sync_id() const;

    [[nodiscard]] std::chrono::steady_clock::time_point get_last_time() const;

    void update_last(boost::uuids::uuid new_sync_id);

  protected:
    std::string name;
    boost::uuids::uuid last_sync_id;
    std::chrono::steady_clock::time_point last_time;
  };

  using logger = std::function<void(const std::string &)>;

  static constexpr size_t version_max_length = 32;
  static constexpr size_t size_max = 4;

  const logger log_error, log_info;

  const std::chrono::minutes lobby_lifetime;
  const std::chrono::minutes game_lifetime;
  const boost::uuids::uuid id;
  const std::string version;
  const std::string name;
  const size_t size;

  [[nodiscard]] explicit room_t(
    std::string version, const user_t &owner, std::string name, size_t size, std::chrono::minutes lobby_lifetime,
    std::chrono::minutes game_lifetime, logger log_error = [](const std::string &) {},
    logger log_info = [](const std::string &) {}
  );

  [[nodiscard]] std::chrono::steady_clock::time_point get_expire_time() const;

  void join(std::string version, const user_t &user);

  [[nodiscard]] user_t get_user(boost::uuids::uuid id) const;

  [[nodiscard]] bool has_user(boost::uuids::uuid id) const;

  bool kick(boost::uuids::uuid id);

  size_t kick_expired(std::chrono::milliseconds timeout);

  [[nodiscard]] size_t count_users() const;

  [[nodiscard]] std::vector<boost::uuids::uuid> get_user_ids() const;

  [[nodiscard]] std::vector<user_t> get_users() const;

  [[nodiscard]] user_t get_owner() const;

  [[nodiscard]] bool is_in_lobby() const;

  void start_game();

  [[nodiscard]] bool is_available() const;

  [[nodiscard]] nlohmann::json get_info() const;

  void update_info(const nlohmann::json &new_info);

  [[nodiscard]] std::vector<std::shared_ptr<sync_record_t>> sync(
    boost::uuids::uuid user_id, const std::vector<std::shared_ptr<sync_record_t::event_t>> &reports,
    const std::vector<std::shared_ptr<sync_record_t::event_t>> &actions,
    std::chrono::milliseconds wait_timeout = std::chrono::milliseconds{ 200 },
    std::chrono::milliseconds sync_timeout = std::chrono::milliseconds{ 50 }
  );

  size_t clean_sync_records();

protected:
  static boost::uuids::uuid gen_id();

  mutable std::shared_mutex room_mutex;
  std::chrono::steady_clock::time_point expire_time;
  std::map<boost::uuids::uuid, user_t> users;
  bool in_lobby;
  nlohmann::json info;
  std::map<boost::uuids::uuid, std::shared_ptr<sync_record_t>> sync_records;
  std::condition_variable_any sync_cv;
};
